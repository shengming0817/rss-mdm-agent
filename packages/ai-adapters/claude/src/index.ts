import type { ProviderAgentPort } from "@rss-mdm-agent/ai-contract";
import { ClaudeAdapter } from "./adapter.js";
import { nativeRuntime } from "./runtime.js";
import type { ClaudeAdapterOptions } from "./configuration.js";
export type {
  ClaudeAdapterOptions,
  ResolvedClaudeConfiguration,
} from "./configuration.js";
export { SDK_VERSION, CLI_VERSION } from "./configuration.js";
/** One adapter owns one native session incarnation; Host owns durable history and queuing. */
export function createClaudeAdapter(
  options: ClaudeAdapterOptions,
): ProviderAgentPort {
  return new ClaudeAdapter(options, nativeRuntime);
}
