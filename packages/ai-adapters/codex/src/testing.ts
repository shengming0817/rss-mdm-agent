import { CodexAdapter } from "./adapter.js";
import type { CodexAdapterOptions } from "./configuration.js";
import type { RuntimeFactory } from "./runtime.js";
export type {
  RuntimeFactory,
  RpcConnection,
  LaunchSpec,
  NativeMessage,
} from "./runtime.js";
/** Protocol fixture seam; never a native model or containment proof. */
export function createTestAdapter(
  options: CodexAdapterOptions,
  runtime: RuntimeFactory,
): CodexAdapter {
  return new CodexAdapter(options, runtime);
}
