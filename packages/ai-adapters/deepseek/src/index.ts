import type { ProviderAgentPort } from "@rss-mdm-agent/ai-contract";
import { DeepSeekAdapter } from "./adapter.js";
import type { DeepSeekAdapterOptions } from "./configuration.js";
export type {
  DeepSeekAdapterOptions,
  DeepSeekConfiguration,
  DeepSeekDiagnostic,
  ResolvedDeepSeekConfiguration,
} from "./configuration.js";
/** The caller owns trusted namespace/configuration resolution and controlled platform verification. */
export function createDeepSeekAdapter(
  options: DeepSeekAdapterOptions,
): ProviderAgentPort {
  return new DeepSeekAdapter(options);
}
