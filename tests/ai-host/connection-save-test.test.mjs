import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createHost, HostFailure } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
const workerRuntime = {
  launcher: "/unused/no-worker-on-save",
  manifestDigest: "0".repeat(64),
};
const caller = { tenantId: "t", principalId: "u", authorityId: "a" };
const budget = () => ({
  timeoutMs: 2000,
  signal: new AbortController().signal,
});
const unwrap = (r) => {
  assert.equal(r.ok, true, r.error?.code);
  return r.value;
};
const draft = {
  connectionId: "offline",
  name: "Offline",
  provider: "codex",
  profile: "conversation",
  source: { type: "existing_config" },
};
test("save persists without resolving a provider; failed testing cannot block subsequent edits", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-save-test-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let resolutions = 0;
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      workerRuntime,
      delivery: null,
      resolve: async () => {
        resolutions++;
        throw new HostFailure({
          code: "authentication_required",
          retry: "never",
        });
      },
    }),
  );
  t.after(async () => {
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const saved = unwrap(
    await host.saveConnection(caller, draft, null, budget()),
  );
  assert.equal(saved.status, "unverified");
  assert.equal(saved.configRevision, 1);
  assert.equal(resolutions, 0);
  const tested = unwrap(
    await host.testConnection(caller, draft.connectionId, 1, budget()),
  );
  assert.equal(tested.lastTest.outcome, "failed");
  assert.equal(tested.lastTest.failure.code, "authentication_required");
  assert.equal(tested.configRevision, 1);
  assert.equal(resolutions, 1);
  const edited = unwrap(
    await host.saveConnection(
      caller,
      { ...draft, name: "Edited" },
      1,
      budget(),
    ),
  );
  assert.equal(edited.configRevision, 2);
  assert.equal(edited.name, "Edited");
  assert.equal(edited.status, "unverified");
  assert.equal(edited.lastTest, null);
  assert.equal(resolutions, 1);
});

test("testing never locks editing and a delayed result cannot overwrite an edit or resurrect deletion", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-test-race-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let started, release;
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      workerRuntime,
      delivery: null,
      resolve: async () => {
        started();
        await new Promise((resolve) => {
          release = resolve;
        });
        throw new HostFailure({
          code: "authentication_required",
          retry: "never",
        });
      },
    }),
  );
  t.after(async () => {
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  unwrap(await host.saveConnection(caller, draft, null, budget()));
  for (const deleting of [false, true]) {
    const revision = deleting ? 2 : 1;
    const entered = new Promise((resolve) => {
      started = resolve;
    });
    const testing = host.testConnection(
      caller,
      draft.connectionId,
      revision,
      budget(),
    );
    await entered;
    const saved = deleting
      ? await host.deleteConnection(
          caller,
          draft.connectionId,
          revision,
          budget(),
        )
      : await host.saveConnection(
          caller,
          { ...draft, name: "New" },
          revision,
          budget(),
        );
    assert.equal(saved.ok, true);
    release();
    assert.equal((await testing).error.code, "revision_conflict");
    assert.deepEqual(
      unwrap(await store.connection(caller, draft.connectionId)),
      saved.value,
    );
  }
});
