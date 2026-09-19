import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  closeAdapters,
  deliverable,
} from "../../../scripts/smoke-deepseek.mjs";
test("official evidence requires unchanged committed source, lock and complete cleanup", () => {
  const source = {
      head: "a",
      base: "b",
      baseOid: "b",
      baseRef: "origin/develop",
      clean: true,
    },
    cleanup = { processesStopped: true, directoryRemoved: true };
  assert.equal(
    deliverable(true, cleanup, source, source, "lock", "lock"),
    true,
  );
  for (const end of [
    { ...source, clean: false },
    { ...source, head: "changed" },
    { ...source, baseOid: "changed" },
  ])
    assert.equal(
      deliverable(true, cleanup, source, end, "lock", "lock"),
      false,
    );
  assert.equal(
    deliverable(true, cleanup, source, source, "lock", "changed"),
    false,
  );
  assert.equal(
    deliverable(
      true,
      { ...cleanup, directoryRemoved: false },
      source,
      source,
      "lock",
      "lock",
    ),
    false,
  );
});
test("smoke cleanup retries failures and still closes later ports before retaining logs", async () => {
  const dir = await mkdtemp(join(tmpdir(), "dsh-smoke-cleanup-"));
  let calls = 0;
  try {
    const cleanup = await closeAdapters(
      [
        {
          close: async () => {
            throw Error("provider secret");
          },
        },
        {
          close: async () => {
            calls++;
            return { ok: true, value: { processStopped: true } };
          },
        },
      ],
      dir,
    );
    assert.equal(calls, 1);
    assert.deepEqual(cleanup, {
      attempts: [2, 1],
      processesStopped: false,
      directoryRemoved: false,
    });
    assert.equal(existsSync(dir), true);
    assert.equal((await closeAdapters([], dir)).directoryRemoved, true);
    assert.equal(existsSync(dir), false);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
