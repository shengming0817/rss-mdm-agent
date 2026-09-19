import type { ProviderAgentPort } from "@rss-mdm-agent/ai-contract";
import { ClaudeAdapter } from "./adapter.js";
import type { ClaudeAdapterOptions } from "./configuration.js";
import type { RuntimeFactory } from "./runtime.js";
export type { RuntimeFactory } from "./runtime.js";
/** Deterministic SDK fixture seam. Never represents real model or containment evidence. */
export function createTestAdapter(
  options: ClaudeAdapterOptions,
  runtime: RuntimeFactory,
): ProviderAgentPort {
  return new ClaudeAdapter(options, runtime);
}
