import { isAbsolute } from "node:path";
import { isIP } from "node:net";
import { lookup } from "node:dns/promises";
import { readPrivateFile } from "./private-file.js";
/** Host bootstrap contains paths only. User identity comes from native ingress. */
export interface LocalConfiguration {
  readonly version: 1;
  readonly databasePath: string;
  readonly nativeDirectory: string;
  readonly workingDirectory: string;
}
export class ConfigurationError extends Error {
  constructor(
    readonly code:
      | "configuration_file"
      | "configuration_invalid"
      | "authentication_required"
      | "unsupported_capability",
  ) {
    super(code);
  }
}
function privateAddress(hostname: string): boolean {
  const host = hostname.replace(/^\[|\]$/g, "").toLowerCase();
  if (host === "localhost") return true;
  if (isIP(host) === 4) {
    const [a, b] = host.split(".").map(Number);
    return (
      a === 0 ||
      a === 10 ||
      a === 127 ||
      (a === 169 && b === 254) ||
      (a === 172 && b >= 16 && b <= 31) ||
      (a === 192 && b === 168) ||
      a >= 224
    );
  }
  return (
    isIP(host) === 6 &&
    (host === "::" ||
      host === "::1" ||
      host.startsWith("fc") ||
      host.startsWith("fd") ||
      /^fe[89ab]/.test(host) ||
      host.startsWith("ff") ||
      host.startsWith("::ffff:"))
  );
}
export function endpoint(value: unknown): string {
  if (typeof value !== "string")
    throw new ConfigurationError("configuration_invalid");
  const url = new URL(value);
  if (
    (url.protocol !== "https:" &&
      !(
        url.protocol === "http:" &&
        ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
      )) ||
    (url.protocol === "https:" && privateAddress(url.hostname)) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new ConfigurationError("configuration_invalid");
  return url.href.replace(/\/$/, "");
}
/** Resolve before handing a custom endpoint to an SDK; every returned address must be public. */
export async function validateEndpointNetwork(value: unknown): Promise<string> {
  const normalized = endpoint(value);
  const url = new URL(normalized);
  if (url.protocol === "http:") return normalized;
  let addresses: { address: string; family: number }[];
  try {
    addresses = await lookup(url.hostname, { all: true, verbatim: true });
  } catch {
    throw new ConfigurationError("configuration_invalid");
  }
  if (
    !addresses.length ||
    addresses.some(({ address }) => privateAddress(address))
  )
    throw new ConfigurationError("configuration_invalid");
  return normalized;
}
export async function readConfiguration(
  path: string,
): Promise<LocalConfiguration> {
  let content: string;
  try {
    content = await readPrivateFile(path, 65536);
  } catch {
    throw new ConfigurationError("configuration_file");
  }
  try {
    const value = JSON.parse(content) as LocalConfiguration;
    const paths = [
      "databasePath",
      "nativeDirectory",
      "workingDirectory",
    ] as const;
    if (
      value.version !== 1 ||
      Object.keys(value).length !== paths.length + 1 ||
      paths.some(
        (key) => typeof value[key] !== "string" || !isAbsolute(value[key]),
      )
    )
      throw new Error();
    return value;
  } catch {
    throw new ConfigurationError("configuration_invalid");
  }
}
