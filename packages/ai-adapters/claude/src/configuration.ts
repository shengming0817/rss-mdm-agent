import { userInfo } from "node:os";
import { readFileSync, lstatSync, mkdirSync, realpathSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import type { Options } from "@anthropic-ai/claude-agent-sdk";
import type {
  Binding,
  Budget,
  Clock,
  ProviderConfiguration,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
// The installed package manifests own compatibility identity, including packed consumers.
const manifest = (url: URL) => JSON.parse(readFileSync(url, "utf8"));
const adapterPackage = manifest(new URL("../package.json", import.meta.url));
const sdkPackage = manifest(
  new URL(
    "./package.json",
    import.meta.resolve("@anthropic-ai/claude-agent-sdk"),
  ),
);
const version = (value: unknown): string => {
  if (typeof value !== "string" || !/^\d+\.\d+\.\d+$/.test(value))
    throw new Error("invalid package version metadata");
  return value;
};
export const ADAPTER_VERSION = version(adapterPackage.version);
export const SDK_VERSION = version(sdkPackage.version);
export const CLI_VERSION = version(sdkPackage.claudeCodeVersion);
if (
  adapterPackage.dependencies["@anthropic-ai/claude-agent-sdk"] !== SDK_VERSION
)
  throw new Error("SDK package version does not match pinned dependency");
export const PROVIDER_VERSION = `claude-agent-sdk-${SDK_VERSION}/claude-code-${CLI_VERSION}`;
export type ClaudeConfiguration = ProviderConfiguration & {
  readonly provider: "claude";
};
/** Secrets come from trusted composition, never a command or serialized binding. */
export interface ResolvedClaudeConfiguration {
  configuration: ClaudeConfiguration;
  configurationDirectory: string;
  apiUrl: string;
  credential:
    | { type: "api_key" | "auth_token" | "oauth_token"; value: string }
    | { type: "existing_login"; secureStorageDirectory: string };
  verifyAccount?: (account: {
    email: string;
    organization: string;
    tokenSource: string;
    apiKeySource: string;
  }) => Promise<void>;
  model?: string;
}
export interface ClaudeAdapterOptions {
  readonly tools?: ToolEndpoint;
  resolveConfiguration(
    identity: Pick<Binding, "config" | "accountRef">,
    budget: Budget,
  ): Promise<ResolvedClaudeConfiguration>;
  clock?: Clock;
  callbackTimeoutMs?: number;
}
/** Native transcripts and resume state share the credential trust boundary. */
function privateDirectory(path: string): string {
  try {
    if (!isAbsolute(path) || !process.getuid) throw new Error();
    mkdirSync(path, { recursive: true, mode: 0o700 });
    const stat = lstatSync(path);
    if (
      !stat.isDirectory() ||
      stat.isSymbolicLink() ||
      stat.uid !== process.getuid() ||
      (stat.mode & 0o077) !== 0
    )
      throw new Error();
    const canonical = realpathSync(path),
      actual = lstatSync(canonical);
    if (actual.dev !== stat.dev || actual.ino !== stat.ino) throw new Error();
    return canonical;
  } catch {
    throw new Error("invalid configuration directory");
  }
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
    (credential.type !== "existing_login" &&
      (!credential.value ||
        !["api_key", "auth_token", "oauth_token"].includes(credential.type))) ||
    !isAbsolute(config.workingDirectory) ||
    !isAbsolute(resolved.configurationDirectory)
  )
    throw new Error("invalid configuration");
  const env: Record<string, string> = {
    ANTHROPIC_BASE_URL: resolved.apiUrl,
    CLAUDE_CONFIG_DIR: privateDirectory(resolved.configurationDirectory),
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
  };
  // No host search path: native metadata helpers must not select user shims.
  env.PATH =
    process.platform === "win32"
      ? join(resolved.configurationDirectory, "empty-bin")
      : "/usr/bin:/bin";
  env.NoDefaultCurrentDirectoryInExePath = "1";
  for (const key of ["SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"])
    if (process.env[key]) env[key] = process.env[key]!;
  if (credential.type === "existing_login") {
    if (
      process.platform !== "darwin" ||
      (credential.secureStorageDirectory !== "" &&
        !isAbsolute(credential.secureStorageDirectory))
    )
      throw new Error("unsupported login source");
    env.CLAUDE_SECURESTORAGE_CONFIG_DIR = credential.secureStorageDirectory;
    if (process.env.HOME) env.HOME = process.env.HOME;
    env.USER = userInfo().username;
    env.LOGNAME = env.USER;
  } else {
    env[
      credential.type === "api_key"
        ? "ANTHROPIC_API_KEY"
        : credential.type === "oauth_token"
          ? "CLAUDE_CODE_OAUTH_TOKEN"
          : "ANTHROPIC_AUTH_TOKEN"
    ] = credential.value;
  }
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
