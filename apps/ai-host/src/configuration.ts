import { readFile, lstat } from "node:fs/promises";
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
/** A private, administrator-owned local composition input; never a protocol message. */
export async function readConfiguration(
  path: string,
): Promise<LocalConfiguration> {
  const stat = await lstat(path);
  if (
    !stat.isFile() ||
    stat.isSymbolicLink() ||
    (stat.mode & 0o077) !== 0 ||
    (process.getuid && stat.uid !== process.getuid()) ||
    stat.size > 65536
  )
    throw new Error("configuration must be a private owned file");
  const value = JSON.parse(await readFile(path, "utf8")) as LocalConfiguration;
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
    !["api_key", "auth_token"].includes(value.claude.credentialType)
  )
    throw new Error("invalid local configuration");
  return value;
}
