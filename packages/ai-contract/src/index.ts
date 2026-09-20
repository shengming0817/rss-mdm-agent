/** V5 product reliability contracts. Data never grants execution authority. */
export type * from "./wire.js";
export type * from "./ports.js";
export {
  decode,
  isId,
  boundedJson,
  fingerprint,
  deliveryFingerprint,
  ContractError,
} from "./codec.js";
export type { Limits, Diagnostic } from "./codec.js";
export {
  extension,
  resolveSurfaceAction,
  projectDelta,
  parseNegotiation,
  selectNegotiation,
} from "./protocol.js";

export type {
  AdmissionResult,
  ForkAdmissionResult,
  VerifiedProviderFact,
} from "./session.js";

export {
  interactionCatalog,
  accessLimits,
  validateSurface,
  SurfaceError,
} from "./a2ui.js";

export { boundedStream } from "./stream.js";
export { withinBudget } from "./budget.js";

export { errorCodes } from "./identity.js";

export {
  activeStage,
  productSession,
  providerStage,
  startStage,
  replaceStage,
} from "./contexts.js";

export {
  emptyPreferences,
  connectionRevision,
  mergePreferences,
} from "./connections.js";

export { historyPreview, promptText } from "./history.js";
