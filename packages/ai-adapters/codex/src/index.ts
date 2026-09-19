import { CodexAdapter, type CodexAdapterPort } from "./adapter.js";
import type { CodexAdapterOptions } from "./configuration.js";
import { nativeRuntime } from "./runtime.js";
export type {
  CodexAdapterPort,
  CodexDiagnostic,
  CodexForkResult,
} from "./adapter.js";
export type {
  CodexAdapterOptions,
  CodexConfiguration,
  ResolvedCodexConfiguration,
} from "./configuration.js";
export { CODEX_VERSION } from "./runtime.js";
export type { Turn as CodexHistoryTurn } from "./protocol.js";
export function createCodexAdapter(
  options: CodexAdapterOptions,
): CodexAdapterPort {
  return new CodexAdapter(options, nativeRuntime);
}
