import {
  createClaudeAdapter,
  type ClaudeAdapterOptions,
  type ResolvedClaudeConfiguration,
} from "../../../packages/ai-adapters/claude/dist/index.js";
import type { ProviderAgentPort } from "../../../packages/ai-contract/dist/index.js";
declare const options: ClaudeAdapterOptions;
const adapter: ProviderAgentPort = createClaudeAdapter(options);
// @ts-expect-error A Claude resolver cannot claim another provider.
const wrongProvider: ResolvedClaudeConfiguration["configuration"]["provider"] =
  "codex";
createClaudeAdapter({
  resolveConfiguration: options.resolveConfiguration,
  // @ts-expect-error SDK permission bypass is not a product option.
  permissionMode: "bypassPermissions",
});
