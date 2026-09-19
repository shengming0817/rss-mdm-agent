import { writeSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  fixtureSession,
  acceptance,
  unwrap,
  dispatchCommand,
} from "../../packages/ai-contract/dist/testing/index.js";
const config = JSON.parse(process.argv[2]);
const tell = (value) => writeSync(1, JSON.stringify(value) + "\n");
const stop = (stage) => {
  tell({ stage });
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
};
if (config.scenario === "migration-before-commit") {
  const exec = DatabaseSync.prototype.exec;
  DatabaseSync.prototype.exec = function (sql) {
    if (sql === "COMMIT") stop("migration-before-commit");
    return exec.call(this, sql);
  };
}
const result = openSqliteStore({
  path: config.path,
  mode: config.mode,
  busyTimeoutMs: 50,
});
if (!result.ok) {
  tell(result);
  process.exit(0);
}
const store = result.value,
  initial = fixtureSession();
initial.capabilities.continuation = "across_processes";
if (config.mode === "create") unwrap(await store.create(initial));
if (config.scenario === "probe") {
  tell({ ok: true, session: unwrap(await store.session(initial.namespace)) });
  unwrap(
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    }),
  );
} else if (config.scenario === "hold") stop("idle-owner");
else {
  if (config.scenario.startsWith("intent") || config.scenario === "unknown")
    unwrap(await store.accept(acceptance(initial)));
  const exec = DatabaseSync.prototype.exec;
  if (config.scenario !== "unknown" && config.scenario !== "before-publish")
    DatabaseSync.prototype.exec = function (sql) {
      if (sql === "COMMIT" && config.scenario.endsWith("before-commit"))
        stop("before-commit");
      const value = exec.call(this, sql);
      if (sql === "COMMIT" && config.scenario.endsWith("after-commit"))
        stop("after-commit");
      return value;
    };
  if (config.scenario.startsWith("intent") || config.scenario === "unknown") {
    const current = unwrap(await store.session(initial.namespace));
    await dispatchCommand(store, current, "command-1", "unknown");
    stop("unknown");
  } else {
    unwrap(await store.accept(acceptance(initial)));
    // The accepted transaction is durable; no stable event has been published.
    stop("before-publish");
  }
}
