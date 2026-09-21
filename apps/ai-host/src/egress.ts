import { randomUUID } from "node:crypto";
import { lookup } from "node:dns/promises";
import {
  createServer,
  request as httpRequest,
  type ClientRequest,
  type IncomingHttpHeaders,
} from "node:http";
import { request as httpsRequest } from "node:https";
import { isIP, type LookupFunction } from "node:net";

type Address = { address: string; family: number };
type Resolve = (hostname: string) => Promise<readonly Address[]>;
const resolveAll: Resolve = (hostname) =>
  lookup(hostname, { all: true, verbatim: true });

const ipv4 = (address: string): readonly number[] | undefined => {
  if (isIP(address) !== 4) return undefined;
  const bytes = address.split(".").map(Number);
  return bytes.length === 4 && bytes.every((byte) => byte <= 255)
    ? bytes
    : undefined;
};
const loopback = (address: string): boolean => {
  const bytes = ipv4(address);
  return bytes ? bytes[0] === 127 : address.toLowerCase() === "::1";
};
const publicAddress = (address: string): boolean => {
  const bytes = ipv4(address);
  if (bytes) {
    const [a, b, c] = bytes;
    return !(
      a === 0 ||
      a === 10 ||
      a === 127 ||
      (a === 100 && b >= 64 && b <= 127) ||
      (a === 169 && b === 254) ||
      (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && b === 0 && c === 0) ||
      (a === 192 && b === 168) ||
      (a === 198 && (b === 18 || b === 19)) ||
      a >= 224
    );
  }
  if (isIP(address) !== 6) return false;
  const first = Number.parseInt(address.split(":", 1)[0] || "0", 16);
  return first >= 0x2000 && first <= 0x3fff;
};
const localEndpoint = (url: URL): boolean =>
  url.protocol === "http:" &&
  ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);

/** Resolve once, reject mixed/private answers, then bind the approved address to the socket. */
export async function resolvePinnedLookup(
  url: URL,
  resolve: Resolve = resolveAll,
): Promise<LookupFunction> {
  const local = localEndpoint(url);
  if (!local && url.protocol !== "https:") throw new Error("egress_rejected");
  const hostname = url.hostname.replace(/^\[|\]$/g, "");
  let addresses: readonly Address[];
  try {
    addresses = await resolve(hostname);
  } catch {
    throw new Error("egress_rejected");
  }
  if (
    addresses.length === 0 ||
    addresses.some(
      ({ address, family }) =>
        ![4, 6].includes(family) ||
        !(local ? loopback(address) : publicAddress(address)),
    )
  )
    throw new Error("egress_rejected");
  const approved = addresses.map(({ address, family }) => ({
    address,
    family,
  }));
  return ((
    name: string,
    options: unknown,
    callback: (...args: any[]) => void,
  ) => {
    if (name !== hostname) {
      callback(new Error("egress_rejected"));
      return;
    }
    if (
      options &&
      typeof options === "object" &&
      "all" in options &&
      options.all
    )
      callback(null, approved);
    else callback(null, approved[0].address, approved[0].family);
  }) as LookupFunction;
}

const upstreamHeaders = (headers: IncomingHttpHeaders) => {
  const copy = { ...headers };
  delete copy.host;
  delete copy.connection;
  delete copy["proxy-connection"];
  delete copy["transfer-encoding"];
  return copy;
};

export interface EgressProxy {
  readonly endpoint: string;
  beginAttempt(id: string): void;
  takeFailure(
    id: string,
  ): import("@rss-mdm-agent/ai-contract").Failure["code"] | undefined;
  endAttempt(id: string): void;
  close(): Promise<void>;
}

/** One fixed-target proxy is the only network authority exposed to provider SDKs. */
export async function startEgressProxy(
  endpoint: string,
  resolve: Resolve = resolveAll,
): Promise<EgressProxy> {
  const target = new URL(endpoint);
  if (
    (!localEndpoint(target) && target.protocol !== "https:") ||
    target.username ||
    target.password ||
    target.search ||
    target.hash
  )
    throw new Error("egress_rejected");
  const route = `/${randomUUID()}`;
  const upstream = new Set<ClientRequest>();
  type Attempt = {
    requests: number;
    pending: boolean;
    ambiguous: boolean;
    consumed: boolean;
    failure?: import("@rss-mdm-agent/ai-contract").Failure["code"];
  };
  const attempts = new Map<string, Attempt>();
  const server = createServer(async (incoming, outgoing) => {
    // Capture the owner when this HTTP request arrives. SDK background/retried
    // requests or overlapping dispatches provide insufficient causal evidence.
    const owner =
      attempts.size === 1 ? attempts.values().next().value : undefined;
    if (owner) {
      owner.requests++;
      owner.pending = true;
    }
    const outcome = (
      failure?: import("@rss-mdm-agent/ai-contract").Failure["code"],
    ) => {
      if (owner) {
        owner.pending = false;
        owner.failure = failure;
      }
    };
    try {
      const requestUrl = new URL(incoming.url ?? "/", "http://localhost");
      if (
        requestUrl.pathname !== route &&
        !requestUrl.pathname.startsWith(`${route}/`)
      ) {
        outcome("invalid_input");
        outgoing.writeHead(404).end();
        return;
      }
      const suffix = requestUrl.pathname.slice(route.length);
      const root = target.pathname.replace(/\/+$/, "");
      const destination = new URL(target);
      destination.pathname = `${root}${suffix || "/"}`;
      destination.search = requestUrl.search;
      const pinned = await resolvePinnedLookup(destination, resolve);
      const send =
        destination.protocol === "https:" ? httpsRequest : httpRequest;
      const forwarded = send(
        destination,
        {
          method: incoming.method,
          headers: upstreamHeaders(incoming.headers),
          lookup: pinned,
          agent: false,
        },
        (response) => {
          outcome(
            response.statusCode === 401
              ? "authentication_required"
              : response.statusCode === 403
                ? "permission_denied"
                : response.statusCode === 429
                  ? "limit_exceeded"
                  : response.statusCode === 400 || response.statusCode === 404
                    ? "invalid_input"
                    : response.statusCode && response.statusCode >= 500
                      ? "unavailable"
                      : undefined,
          );
          response.once("error", () => {
            outcome("unavailable");
            outgoing.destroy();
          });
          if (
            response.statusCode !== undefined &&
            response.statusCode >= 300 &&
            response.statusCode < 400
          ) {
            outcome("unavailable");
            response.resume();
            outgoing.writeHead(502).end();
            return;
          }
          outgoing.writeHead(
            response.statusCode ?? 502,
            response.statusMessage,
            response.rawHeaders,
          );
          response.pipe(outgoing);
        },
      );
      upstream.add(forwarded);
      forwarded.once("close", () => upstream.delete(forwarded));
      incoming.once("aborted", () => forwarded.destroy());
      forwarded.once("error", () => {
        outcome("unavailable");
        if (!outgoing.headersSent) outgoing.writeHead(502);
        outgoing.end();
      });
      incoming.pipe(forwarded);
    } catch {
      outcome("unavailable");
      if (!outgoing.headersSent) outgoing.writeHead(502);
      outgoing.end();
    }
  });
  await new Promise<void>((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
      resolveListen();
    });
  });
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("egress_rejected");
  let closed: Promise<void> | undefined;
  return {
    beginAttempt(id) {
      if (attempts.has(id)) return;
      if (attempts.size >= 16) throw new Error("egress_attempt_unavailable");
      for (const active of attempts.values()) active.ambiguous = true;
      attempts.set(id, {
        requests: 0,
        pending: false,
        ambiguous: attempts.size > 0,
        consumed: false,
      });
    },
    takeFailure(id) {
      const owner = attempts.get(id);
      if (!owner || owner.consumed) return;
      owner.consumed = true;
      if (!owner.ambiguous && owner.requests === 1 && !owner.pending)
        return owner.failure;
    },
    endAttempt(id) {
      attempts.delete(id);
    },
    endpoint: `http://127.0.0.1:${address.port}${route}`,
    close: () =>
      (closed ??= new Promise<void>((resolveClose, reject) => {
        attempts.clear();
        server.closeAllConnections();
        for (const request of upstream) request.destroy();
        server.close((error) => (error ? reject(error) : resolveClose()));
      })),
  };
}
