import { homedir } from "node:os";
import { isAbsolute, join } from "node:path";
import {
  ConfigurationError,
  endpoint,
  type LocalConfiguration,
} from "./configuration.js";
import type { Connection, Namespace } from "@rss-mdm-agent/ai-contract";
import type { ResolvedCodexConfiguration } from "@rss-mdm-agent/ai-adapter-codex";
import type { ResolvedClaudeConfiguration } from "@rss-mdm-agent/ai-adapter-claude";
/** Private activation data travels only over the already-owned worker descriptor. */
export interface ProviderActivation {
  local: Pick<LocalConfiguration, "nativeDirectory" | "workingDirectory">;
  connection: Connection;
  namespace: Namespace;
  secret?: string;
  verification?: true;
}
export interface ResolvedConnection {
  model?: string;
  apiUrl?: string;
  apiKey?: string;
  codex?: ResolvedCodexConfiguration["authentication"];
  claude?: ResolvedClaudeConfiguration["authentication"];
}
/** The official CLI/SDK owns login/config resolution, refresh and account identity. */
export function resolveConnection({
  connection,
  secret,
}: ProviderActivation): ResolvedConnection {
  const source = connection.source;
  if (source.type === "custom_api") {
    if (!secret?.trim() || Buffer.byteLength(secret) > 16384)
      throw new ConfigurationError("authentication_required");
    const apiUrl = endpoint(source.apiUrl);
    return {
      model: source.model,
      apiUrl,
      apiKey: secret,
      codex: { type: "api_key", apiUrl, apiKey: secret },
      claude: {
        type: "custom_api",
        apiUrl,
        credential: { type: source.credentialType ?? "api_key", value: secret },
      },
    };
  }
  if (connection.provider === "deepseek")
    throw new ConfigurationError("configuration_invalid");
  const directory =
    source.directory ??
    join(homedir(), connection.provider === "codex" ? ".codex" : ".claude");
  if (!isAbsolute(directory))
    throw new ConfigurationError("configuration_invalid");
  return {
    model: source.model,
    codex: {
      type: "existing_config",
      directory,
    },
    claude: { type: "existing_config", directory },
  };
}
