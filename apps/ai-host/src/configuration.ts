import { isAbsolute } from "node:path";
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
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new ConfigurationError("configuration_invalid");
  return url.href.replace(/\/$/, "");
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
