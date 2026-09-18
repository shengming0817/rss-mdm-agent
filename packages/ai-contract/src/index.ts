/** V2 product reliability contracts. Data never grants execution authority. */
export type * from "./wire.js";
export type * from "./ports.js";
export {
  decode,
  fingerprint,
  deliveryFingerprint,
  ContractError,
} from "./codec.js";
export type { Limits, Diagnostic } from "./codec.js";
export { extension, resolveSurfaceAction } from "./protocol.js";
