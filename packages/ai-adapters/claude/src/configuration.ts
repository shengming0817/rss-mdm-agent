import { isAbsolute } from "node:path";
import type { Options } from "@anthropic-ai/claude-agent-sdk";
import type {
  Binding,
  Budget,
  Clock,
  ProviderConfiguration,
} from "@rss-mdm-agent/ai-contract";
export const SDK_VERSION = "0.3.277";
export const CLI_VERSION = "2.1.277";
export const PROVIDER_VERSION = `claude-agent-sdk-${SDK_VERSION}/claude-code-${CLI_VERSION}`;
/** Secrets come from trusted composition, never a command or serialized binding. */
export interface ResolvedClaudeConfiguration {
  configuration: ProviderConfiguration;
  configurationDirectory: string;
  apiUrl: string;
  credential: { type: "api_key" | "auth_token"; value: string };
  model?: string;
}
export interface ClaudeAdapterOptions {
  resolveConfiguration(
    identity: Pick<Binding, "config" | "accountRef">,
    budget: Budget,
  ): Promise<ResolvedClaudeConfiguration>;
  clock?: Clock;
  callbackTimeoutMs?: number;
}
/** No raw SDK option passthrough. Settings, child environment and tool inventory are sealed. */
export function sdkOptions(resolved: ResolvedClaudeConfiguration): Options {
  const { configuration: config, credential } = resolved;
  const url = new URL(resolved.apiUrl);
  if (
    url.protocol !== "https:" &&
    !(
      url.protocol === "http:" &&
      ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
    )
  )
    throw new Error("invalid configuration");
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !credential.value ||
    !["api_key", "auth_token"].includes(credential.type) ||
    !isAbsolute(config.workingDirectory) ||
    !isAbsolute(resolved.configurationDirectory)
  )
    throw new Error("invalid configuration");
  const env: Record<string, string> = {
    ANTHROPIC_BASE_URL: resolved.apiUrl,
    CLAUDE_CONFIG_DIR: resolved.configurationDirectory,
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
  };
  for (const key of ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"])
    if (process.env[key]) env[key] = process.env[key]!;
  env[
    credential.type === "api_key" ? "ANTHROPIC_API_KEY" : "ANTHROPIC_AUTH_TOKEN"
  ] = credential.value;
  return {
    cwd: config.workingDirectory,
    env,
    ...(resolved.model ? { model: resolved.model } : {}),
    tools: ["AskUserQuestion"],
    allowedTools: [],
    permissionMode: "default",
    allowDangerouslySkipPermissions: false,
    settingSources: [],
    settings: {
      disableBundledSkills: true,
      skillOverrides: { doctor: "off" },
      disableSkillShellExecution: true,
      enableAllProjectMcpServers: false,
      disableClaudeAiConnectors: true,
    },
    skills: [],
    plugins: [],
    strictMcpConfig: true,
    mcpServers: {},
    agents: {},
    additionalDirectories: [],
    includePartialMessages: true,
    persistSession: true,
    enableFileCheckpointing: false,
    maxTurns: 32,
    stderr: () => {},
    onElicitation: async () => ({ action: "cancel" }),
    onUserDialog: async () => ({ behavior: "cancelled" }),
    supportedDialogKinds: [],
  };
}
