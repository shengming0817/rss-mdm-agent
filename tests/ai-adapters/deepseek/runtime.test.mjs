import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { existsSync } from "node:fs";
import { nativeRuntime } from "../../../packages/ai-adapters/deepseek/dist/runtime.js";
import { budget } from "./support.mjs";
for (const mode of ["throw", "error-without-exit"])
  test(`spawn failure cleans owned home: ${mode}`, async () => {
    let home;
    const child = new EventEmitter();
    child.kill = () => true;
    const runtime = nativeRuntime((_url, _args, options) => {
      home = options.env.DSH_HOME;
      if (mode === "throw") throw Error("secret must not escape");
      queueMicrotask(() => {
        child.emit("error", Error("secret"));
        child.emit("close", -2);
      });
      return child;
    });
    const events = [];
    runtime.onEvent((e) => events.push(e));
    try {
      await assert.rejects(runtime.call("initialize", {}, budget(50)));
      await Promise.race([
        runtime.stopped,
        new Promise((_, reject) =>
          setTimeout(() => reject(Error("cleanup timed out")), 100),
        ),
      ]);
      assert.equal(existsSync(home), false);
      assert.ok(events.some((e) => e.diagnostic?.reason === "spawn_failed"));
      assert.ok(!JSON.stringify(events).includes("secret"));
    } finally {
      runtime.stop();
    }
  });

test("worker executable search never inherits a hostile host PATH", async () => {
  const original = process.env.PATH;
  let environment;
  process.env.PATH = "/hostile/shims";
  try {
    const runtime = nativeRuntime((_url, _args, options) => {
      environment = options.env;
      throw new Error("no process needed");
    });
    await runtime.stopped;
    assert.notEqual(environment.PATH, "/hostile/shims");
    assert.equal(environment.NoDefaultCurrentDirectoryInExePath, "1");
  } finally {
    if (original === undefined) delete process.env.PATH;
    else process.env.PATH = original;
  }
});
