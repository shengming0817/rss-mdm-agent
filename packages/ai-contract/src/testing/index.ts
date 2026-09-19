/** Deterministic test support only; no production durability or security claim. */
export { MemorySessionStore, fixtureLimits } from "./store.js";
export { FakeHost } from "./host.js";
export {
  runStoreConformance,
  seedInteraction,
  seedSurface,
  terminalCommit,
  fixtureCaller,
  fixtureSession,
  fixtureCommand,
  acceptance,
  emptyCommit,
  unwrap,
} from "./conformance.js";
export { fixtures } from "./fixtures.js";
export { ScriptedProvider, runProviderConformance } from "./provider.js";
export { runHostConformance } from "./host-conformance.js";
