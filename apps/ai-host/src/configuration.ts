import { readPrivateFile } from "./private-file.js";
import { isAbsolute } from "node:path";
import {
  isId,
  type Caller,
  type SessionOptions,
} from "@rss-mdm-agent/ai-contract";

export type ConnectionSource =
  | {
      source: "existing_user_config";
      directory: string;
      model?: string;
      profile?: string;
    }
  | {
      source: "custom_endpoint";
      apiUrl: string;
      credentialPath: string;
      credentialType: "api_key" | "auth_token" | "oauth_token";
      model: string;
    };
export interface LocalConfiguration {
  readonly databasePath: string;
  readonly socketPath: string;
  readonly nativeDirectory: string;
  readonly caller: Caller;
  readonly session: SessionOptions;
  readonly workingDirectory: string;
  readonly connection: ConnectionSource;
}
export class ConfigurationError extends Error {
  constructor(
    readonly code:
      | "configuration_file"
      | "configuration_invalid"
      | "authentication_required",
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
const keys = (value: object, allowed: string[]) =>
  Object.keys(value).every((key) => allowed.includes(key));
const model = (value: unknown) =>
  typeof value === "string" && value.trim().length > 0 && value.length <= 256;
/** One exact composition input. Old provider-specific configuration is rejected. */
export async function readConfiguration(
  path: string,
): Promise<LocalConfiguration> {
  let contents: string;
  try {
    contents = await readPrivateFile(path, 65536);
  } catch {
    throw new ConfigurationError("configuration_file");
  }
  try {
    const value = JSON.parse(contents) as LocalConfiguration;
    if (
      !keys(value, [
        "databasePath",
        "socketPath",
        "nativeDirectory",
        "caller",
        "session",
        "workingDirectory",
        "connection",
      ]) ||
      [
        value.databasePath,
        value.socketPath,
        value.nativeDirectory,
        value.workingDirectory,
      ].some((p) => typeof p !== "string" || !isAbsolute(p)) ||
      !value.caller ||
      ![
        value.caller.tenantId,
        value.caller.principalId,
        value.caller.authorityId,
        value.session?.accountRef,
        value.session?.config?.id,
        value.session?.config?.revision,
      ].every(isId) ||
      !["claude", "codex", "deepseek"].includes(value.session.provider) ||
      !["conversation", "controlled_tools"].includes(value.session.profile)
    )
      throw new Error();
    const connection = value.connection;
    if (connection.source === "existing_user_config") {
      if (
        !keys(connection, ["source", "directory", "model", "profile"]) ||
        !isAbsolute(connection.directory) ||
        (connection.model !== undefined && !model(connection.model)) ||
        (connection.profile !== undefined && !isId(connection.profile))
      )
        throw new Error();
    } else if (connection.source === "custom_endpoint") {
      if (
        !keys(connection, [
          "source",
          "apiUrl",
          "credentialPath",
          "credentialType",
          "model",
        ]) ||
        !isAbsolute(connection.credentialPath) ||
        !model(connection.model) ||
        !["api_key", "auth_token", "oauth_token"].includes(
          connection.credentialType,
        ) ||
        (value.session.provider !== "claude" &&
          connection.credentialType !== "api_key")
      )
        throw new Error();
      endpoint(connection.apiUrl);
    } else throw new Error();
    return value;
  } catch {
    throw new ConfigurationError("configuration_invalid");
  }
}
