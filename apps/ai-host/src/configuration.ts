import { readPrivateFile } from "./private-file.js";
import { isAbsolute } from "node:path";
import {
  isId,
  type Caller,
  type SessionOptions,
} from "@rss-mdm-agent/ai-contract";
export interface LocalConfiguration {
  readonly databasePath: string;
  readonly socketPath: string;
  readonly caller: Caller;
  readonly session: SessionOptions;
  readonly workingDirectory: string;
  readonly claude: {
    configurationDirectory: string;
    apiUrl: string;
    credentialPath: string;
    credentialType: "api_key" | "auth_token";
    model?: string;
  };
}
export class ConfigurationError extends Error {
  constructor(readonly code: "configuration_file" | "configuration_invalid") {
    super(code);
  }
}
/** A private, administrator-owned local composition input; never a protocol message. */
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
      [
        value.databasePath,
        value.socketPath,
        value.workingDirectory,
        value.claude?.configurationDirectory,
        value.claude?.credentialPath,
      ].some((path) => typeof path !== "string" || !isAbsolute(path)) ||
      !value.caller ||
      [
        value.caller.tenantId,
        value.caller.principalId,
        value.caller.authorityId,
        value.session?.accountRef,
        value.session?.config?.id,
        value.session?.config?.revision,
      ].some((id) => !isId(id)) ||
      value.session.provider !== "claude" ||
      value.session.profile !== "conversation" ||
      !["api_key", "auth_token"].includes(value.claude.credentialType) ||
      typeof value.claude.apiUrl !== "string" ||
      (value.claude.model !== undefined &&
        (typeof value.claude.model !== "string" ||
          !value.claude.model.trim() ||
          value.claude.model.length > 256))
    )
      throw new Error("invalid local configuration");
    const url = new URL(value.claude.apiUrl);
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
      throw new Error("invalid local configuration");
    return value;
  } catch {
    throw new ConfigurationError("configuration_invalid");
  }
}
