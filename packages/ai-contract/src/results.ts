import type { Limits } from "./codec.js";
import type { Result } from "./ports.js";
export const defaultLimits: Readonly<Limits> = Object.freeze({
  maxBytes: 262144,
  maxTextBytes: 131072,
  maxDepth: 32,
  maxNodes: 16384,
});
export const ok = <T>(value: T): Result<T> => ({ ok: true, value });
export const fail = <T = never>(
  code: import("./wire.js").ErrorCode,
  retry: import("./wire.js").Retry = "never",
): Result<T> => ({ ok: false, error: { code, retry } });
