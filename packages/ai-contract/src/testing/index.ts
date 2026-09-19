/** Deterministic test support only; no production durability or security claim. */
export { MemorySessionStore, fixtureLimits } from "./store.js";
export { FakeHost } from "./host.js";
export {
  runStoreConformance,
  dispatchCommand,
  commandCommit,
  surfaceCommit,
  interactionEvent,
  seedInteraction,
  seedSurface,
  terminalCommit,
  fixtureCaller,
  fixtureSession,
  fixtureCommand,
  fixtureAttempt,
  fixtureDispatchedRecord,
  fixtureProviderSession,
  acceptance,
  emptyCommit,
  unwrap,
} from "./conformance.js";
export { fixtures } from "./fixtures.js";
export { ScriptedProvider, runProviderConformance } from "./provider.js";
export { runHostConformance } from "./host-conformance.js";

export { restoredSession, verifiedReconciliation } from "./recovery.js";
