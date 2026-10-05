import {
  readView,
  resultView,
} from "../../packages/ai-contract/dist/session-mutation.js";
import test from "node:test";
import assert from "node:assert/strict";
import {
  createState,
  acceptCommand,
  fail,
  ok,
} from "../../packages/ai-contract/dist/transitions.js";
import {
  fixtureSession,
  acceptance,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
test("transaction read failures retain their storage classification and leave state unchanged", () => {
  const session = fixtureSession(),
    state = unwrap(createState(session));
  state.source = resultView(readView(state), (read) => ok(read()));
  state.source.hasEvent = () => fail("storage_corrupt");
  const before = structuredClone({ ...state, source: undefined });
  const result = acceptCommand(state, acceptance(session));
  assert.deepEqual(result, fail("storage_corrupt"));
  assert.deepEqual({ ...state, source: undefined }, before);
});
