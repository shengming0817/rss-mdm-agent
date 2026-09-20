import { mkdir, realpath, lstat } from "node:fs/promises";
import { isAbsolute, join } from "node:path";
import type {
  Binding,
  Budget,
  Clock,
  ToolEndpoint,
  ProviderConfiguration,
} from "@rss-mdm-agent/ai-contract";
import { CODEX_VERSION, type LaunchSpec } from "./runtime.js";
import { HOST_SERVER, type ToolBridge } from "./bridge.js";

export const ADAPTER_VERSION = "0.1.0";
/** Internal admission signal for deterministic configuration rejection. */
export class CodexConfigurationFailure extends Error {
  constructor(readonly code: "invalid_input" | "permission_denied") {
    super("Codex configuration rejected");
    this.name = "CodexConfigurationFailure";
  }
}
export type CodexConfiguration = ProviderConfiguration & {
  readonly provider: "codex";
};
/** Trusted host resolution only. Existing configuration stays owned by the official CLI; runtime files belong to RSS. */
export interface ResolvedCodexConfiguration {
  configuration: CodexConfiguration;
  nativeDirectory: string;
  authentication:
    | { type: "api_key"; apiUrl: string; apiKey: string }
    | { type: "existing_config"; directory: string };
  verification?: boolean;
  /** Omit to use the pinned Codex runtime default. */
  model?: string;
  /** Host-owned product instructions; never loaded from native user permissions/settings. */
  developerInstructions?: string;
  /** Returned only after the host finds the requested native IDs in its trusted lineage records. */
  ownedHistory?: { nativeSessionId: string; nativeThreadId: string };
}
export interface CodexAdapterOptions {
  readonly tools?: ToolEndpoint;
  resolveConfiguration(
    identity: Pick<
      ProviderConfiguration,
      "namespace" | "provider" | "config"
    > & { history?: Binding },
    budget: Budget,
  ): Promise<ResolvedCodexConfiguration>;
  clock?: Clock;
  /** Opt in to bounded, lossy, closed diagnostic metadata; no native payloads. */
  nativeDiagnostics?: boolean;
}
export const disabledFeatures = [
  "shell_tool",
  "unified_exec",
  "unified_exec_tty",
  "shell_snapshot",
  "code_mode",
  "code_mode_host",
  "code_mode_only",
  "multi_agent",
  "multi_agent_v2",
  "apps",
  "enable_mcp_apps",
  "hooks",
  "plugins",
  "plugin_sharing",
  "remote_plugin",
  "browser_use",
  "browser_use_external",
  "browser_use_full_cdp_access",
  "computer_use",
  "image_generation",
  "view_image",
  "standalone_web_search",
  "web_search_request",
  "web_search_cached",
  "skill_search",
  "skill_mcp_dependency_install",
  "tool_call_mcp_elicitation",
  "request_permissions_tool",
  "goals",
  "sleep_tool",
  "memories",
  "worktrees",
  "recommended_plugins",
  "tool_suggest",
  "auth_elicitation",
] as const;
export const toml = (value: any): string =>
  Array.isArray(value)
    ? `[${value.map(toml).join(",")}]`
    : value && typeof value === "object"
      ? `{${Object.entries(value)
          .map(([key, v]) => `${JSON.stringify(key)}=${toml(v)}`)
          .join(",")}}`
      : JSON.stringify(value);
export function nativeSettings(
  resolved: ResolvedCodexConfiguration,
): Record<string, unknown> {
  return {
    ...(resolved.model === undefined ? {} : { model: resolved.model }),
    ...(resolved.authentication.type === "api_key"
      ? {
          model_provider: "rss_host_model",
          model_providers: {
            rss_host_model: {
              name: "RSS host model",
              base_url: resolved.authentication.apiUrl,
              env_key: "RSS_CODEX_API_KEY",
              wire_api: "responses",
              requires_openai_auth: false,
              request_max_retries: 0,
              stream_max_retries: 0,
              stream_idle_timeout_ms: 30000,
            },
          },
        }
      : {}),
    approval_policy: "on-request",
    sandbox_mode: "read-only",
    web_search: "disabled",
    allow_login_shell: false,
    check_for_update_on_startup: false,
    project_doc_max_bytes: 0,
    project_doc_fallback_filenames: [],
    include_permissions_instructions: false,
    include_apps_instructions: false,
    include_collaboration_mode_instructions: false,
    include_environment_context: false,
    analytics: { enabled: false },
    feedback: { enabled: false },
    tools: {
      experimental_request_user_input: { enabled: false },
      update_plan: { enabled: false },
    },
    skills: { include_instructions: false, bundled: { enabled: false } },
    orchestrator: { skills: { enabled: false }, mcp: { enabled: false } },
    features: {
      ...Object.fromEntries(disabledFeatures.map((key) => [key, false])),
      skip_host_skill_discovery: true,
    },
    mcp_servers: {},
  };
}
export async function launchSpec(
  resolved: ResolvedCodexConfiguration,
  bridge?: ToolBridge,
): Promise<{
  spec: LaunchSpec;
  settings: Record<string, unknown>;
  overrides: Record<string, unknown>;
}> {
  let url: URL;
  try {
    url = new URL(
      resolved.authentication.type === "api_key"
        ? resolved.authentication.apiUrl
        : "https://api.openai.com/v1",
    );
  } catch {
    throw new CodexConfigurationFailure("invalid_input");
  }
  if (
    (url.protocol !== "https:" &&
      !(
        url.protocol === "http:" &&
        ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
      )) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !(resolved.authentication.type === "api_key"
      ? resolved.authentication.apiKey
      : resolved.authentication.type === "existing_config" &&
        isAbsolute(resolved.authentication.directory)) ||
    (resolved.model !== undefined &&
      (typeof resolved.model !== "string" || !resolved.model.trim())) ||
    !isAbsolute(resolved.nativeDirectory) ||
    !isAbsolute(resolved.configuration.workingDirectory)
  )
    throw new CodexConfigurationFailure("invalid_input");
  await mkdir(resolved.nativeDirectory, { recursive: true, mode: 0o700 });
  const stat = await lstat(resolved.nativeDirectory);
  if (
    !stat.isDirectory() ||
    stat.isSymbolicLink() ||
    (process.platform !== "win32" && stat.mode & 0o077)
  )
    throw new CodexConfigurationFailure("permission_denied");
  if ((await realpath(resolved.nativeDirectory)) !== resolved.nativeDirectory)
    throw new CodexConfigurationFailure("permission_denied");
  const settings = nativeSettings(resolved);
  const overrides = bridge
    ? {
        mcp_servers: {
          [HOST_SERVER]: {
            url: bridge.url!,
            bearer_token_env_var: "RSS_CODEX_MCP_TOKEN",
            enabled: true,
            required: true,
            enabled_tools: ["propose"],
            default_tools_approval_mode: "approve",
            startup_timeout_sec: 10,
            tool_timeout_sec: 30,
          },
        },
      }
    : {};
  const env: Record<string, string> = {
    CODEX_HOME:
      resolved.authentication.type === "existing_config"
        ? resolved.authentication.directory
        : resolved.nativeDirectory,
    ...(resolved.authentication.type === "api_key"
      ? { RSS_CODEX_API_KEY: resolved.authentication.apiKey }
      : {}),
  };
  // No host search path: native metadata helpers must not select user shims.
  env.PATH =
    process.platform === "win32"
      ? join(resolved.nativeDirectory, "empty-bin")
      : "/usr/bin:/bin";
  env.NoDefaultCurrentDirectoryInExePath = "1";
  for (const key of ["HOME", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"])
    if (process.env[key]) env[key] = process.env[key]!;
  if (bridge) env.RSS_CODEX_MCP_TOKEN = bridge.token;
  const args = ["app-server", "--stdio", "--strict-config"];
  for (const [key, value] of Object.entries({ ...settings, ...overrides }))
    args.push("-c", `${key}=${toml(value)}`);
  const runtimeDirectory = join(resolved.nativeDirectory, "runtime-workspace");
  await mkdir(runtimeDirectory, { mode: 0o700, recursive: true });
  return { spec: { cwd: runtimeDirectory, env, args }, settings, overrides };
}
export function compatible(binding: Binding): boolean {
  return (
    binding.provider === "codex" &&
    binding.providerVersion === CODEX_VERSION &&
    binding.adapterVersion === ADAPTER_VERSION &&
    typeof binding.nativeThreadId === "string"
  );
}
