import { test } from "node:test";
import assert from "node:assert/strict";
import { cargoTargetDir } from "./cargo-target.mjs";

test("Cargo target resolver accepts either supported environment variable", () => {
  const root = "/tmp/agent-cargo-target-test";
  assert.equal(cargoTargetDir(root, {}), `${root}/target`);
  assert.equal(
    cargoTargetDir(root, { CARGO_BUILD_TARGET_DIR: "build-output" }),
    `${root}/build-output`,
  );
  assert.equal(
    cargoTargetDir(root, { CARGO_TARGET_DIR: "/tmp/explicit-target" }),
    "/tmp/explicit-target",
  );
  assert.equal(
    cargoTargetDir(root, {
      CARGO_TARGET_DIR: "build-output",
      CARGO_BUILD_TARGET_DIR: "build-output",
    }),
    `${root}/build-output`,
  );
  assert.throws(
    () =>
      cargoTargetDir(root, {
        CARGO_TARGET_DIR: "/tmp/one",
        CARGO_BUILD_TARGET_DIR: "/tmp/two",
      }),
    /conflicting Cargo target directories/,
  );
  assert.throws(
    () => cargoTargetDir(root, { CARGO_BUILD_TARGET_DIR: "" }),
    /must not be empty/,
  );
});
