import type { ProviderInstance } from "@rss-mdm-agent/ai-contract";
import { CodexAdapter } from "./adapter.js";
import type { CodexAdapterOptions } from "./configuration.js";
import { nativeRuntime } from "./runtime.js";
export type {
  CodexAdapterOptions,
  CodexConfiguration,
  ResolvedCodexConfiguration,
} from "./configuration.js";
export { CODEX_VERSION } from "./runtime.js";
export function createCodexAdapter(
  options: CodexAdapterOptions,
): ProviderInstance {
  return new CodexAdapter(options, nativeRuntime).ports();
}
