import { createServer } from "node:http";
import { request } from "node:https";
import { rootCertificates } from "node:tls";
import { randomBytes, createHash } from "node:crypto";

const sha256 = (value) => createHash("sha256").update(value).digest("hex");
/** A claimed model name is evidence only on a response received from the pinned TLS peer. */
export function modelReceipt(event, expectedModel) {
  const response = event?.response;
  if (
    event?.type !== "response.completed" ||
    response?.status !== "completed" ||
    response?.model !== expectedModel ||
    typeof response?.id !== "string" ||
    !/^resp_[A-Za-z0-9_-]+$/.test(response.id)
  )
    throw new Error("backend identity mismatch");
  return {
    backend: "openai",
    modelSha256: sha256(response.model),
    responseIdSha256: sha256(response.id),
  };
}

/** Smoke-only relay: explicit TLS trust, no redirect, no proxy, no user-supplied backend.
 * It binds evidence to the exact native model requests, not a separate probe. */
export async function startSmokeModelGateway(settings) {
  if (
    settings.mode !== "real_model" ||
    settings.apiUrl !== "https://api.openai.com/v1"
  )
    throw new Error("untrusted smoke backend");
  const token = randomBytes(32).toString("hex"),
    receipts = [],
    pending = new Set(),
    settled = [];
  const server = createServer(async (incoming, outgoing) => {
    let upstream;
    try {
      if (
        incoming.method !== "POST" ||
        incoming.url !== "/v1/responses" ||
        incoming.headers.authorization !== `Bearer ${token}`
      ) {
        outgoing.writeHead(403).end();
        return;
      }
      const chunks = [];
      let bytes = 0;
      for await (const chunk of incoming) {
        bytes += chunk.length;
        if (bytes > 4 * 1024 * 1024) throw new Error("request limit");
        chunks.push(chunk);
      }
      const body = Buffer.concat(chunks),
        submitted = JSON.parse(body);
      if (submitted.model !== settings.model || submitted.stream !== true)
        throw new Error("request identity mismatch");
      const receipt = { verified: false };
      receipts.push(receipt);
      let finish;
      settled.push(
        new Promise((resolve) => {
          finish = resolve;
        }),
      );
      upstream = request(
        "https://api.openai.com/v1/responses",
        {
          method: "POST",
          rejectUnauthorized: true,
          ca: rootCertificates,
          headers: {
            authorization: `Bearer ${settings.apiKey}`,
            "content-type": "application/json",
            accept: "text/event-stream",
            "accept-encoding": "identity",
            "content-length": body.length,
          },
          signal: AbortSignal.timeout(90000),
        },
        async (response) => {
          try {
            if (
              response.statusCode !== 200 ||
              !response.headers["content-type"]?.startsWith("text/event-stream")
            )
              throw new Error("upstream rejected");
            // Preserve streaming. Only the bounded metadata projection survives this request.
            outgoing.writeHead(200, { "content-type": "text/event-stream" });
            response.setEncoding("utf8");
            let buffer = "",
              responseBytes = 0,
              completion;
            for await (const chunk of response) {
              responseBytes += Buffer.byteLength(chunk);
              if (responseBytes > 8 * 1024 * 1024)
                throw new Error("response limit");
              buffer += chunk;
              let newline;
              while ((newline = buffer.indexOf("\n")) >= 0) {
                const line = buffer.slice(0, newline).trimEnd();
                buffer = buffer.slice(newline + 1);
                if (!line.startsWith("data:")) continue;
                const data = line.slice(5).trim();
                if (data === "[DONE]") continue;
                const event = JSON.parse(data);
                if (
                  ["error", "response.failed", "response.incomplete"].includes(
                    event.type,
                  )
                )
                  throw new Error("upstream failure");
                if (event.type === "response.completed") {
                  if (completion)
                    throw new Error("duplicate response identity");
                  completion = modelReceipt(event, settings.model);
                }
              }
              outgoing.write(chunk);
            }
            if (!completion || buffer.trim())
              throw new Error("missing response identity");
            Object.assign(receipt, completion, { verified: true });
            outgoing.end();
          } catch {
            response.destroy();
            outgoing.destroy();
          } finally {
            pending.delete(upstream);
            finish();
          }
        },
      );
      pending.add(upstream);
      upstream.on("error", () => {
        pending.delete(upstream);
        outgoing.destroy();
        finish();
      });
      outgoing.on("close", () => {
        if (!outgoing.writableFinished) upstream.destroy();
      });
      upstream.end(body);
    } catch {
      upstream?.destroy();
      outgoing.destroy();
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  return {
    apiUrl: `http://127.0.0.1:${server.address().port}/v1`,
    apiKey: token,
    receipts,
    async verify() {
      await Promise.all(settled);
      return (
        receipts.length === 3 &&
        receipts.every((receipt) => receipt.verified) &&
        new Set(receipts.map((receipt) => receipt.responseIdSha256)).size === 3
      );
    },
    async close() {
      for (const upstream of pending) upstream.destroy();
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}

export function continuityChallenge() {
  const nonce = randomBytes(24).toString("hex");
  return {
    expected: nonce,
    prompts: [
      `Remember this session marker: ${nonce}. Reply with exactly the marker and nothing else.`,
      "Reply with exactly the session marker from the first user message and nothing else.",
      "Recall the session marker from the first user message. Reply with exactly that marker and nothing else.",
    ],
  };
}
