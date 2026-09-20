import assert from "node:assert/strict";
import test from "node:test";
import { fork } from "node:child_process";
import { once } from "node:events";
import { groupEmpty } from "../../packages/ai-host/dist/process.js";
import { openHost } from "./host.mjs";
import {
  engines,
  fixture,
  command,
  until,
  budget,
  unwrap,
  evidence,
} from "../ai-provider-conformance/support.mjs";

for (const provider of engines) {
  test(
    `${provider}: native acceptance survives Host crash before its fact commit without redispatch`,
    { timeout: 90000 },
    async (t) => {
      let child, ready;
      // Register before fixture teardown so no live child outlives its files.
      t.after(async () => {
        if (child && child.exitCode === null && child.signalCode === null) {
          const exit = once(child, "exit");
          child.kill("SIGKILL");
          await exit;
        }
        for (const launch of ready?.launches ?? [])
          await until(
            () => groupEmpty(launch.rootPid),
            "old worker group exit",
          );
      });
      const f = await fixture(t, provider);
      if (provider === "claude") f.model.replies.push("hang");
      else f.model.hold();
      child = fork(new URL("./crash-driver.mjs", import.meta.url), [f.path], {
        stdio: ["ignore", "ignore", "pipe", "ipc"],
      });
      let stderr = "";
      child.stderr.on("data", (data) => {
        if (stderr.length < 8192) stderr += data;
      });
      ready = await new Promise((resolve, reject) => {
        const timer = setTimeout(
          () => reject(Error("crash barrier timeout: " + stderr)),
          30000,
        );
        child.once("message", (value) => {
          clearTimeout(timer);
          resolve(value);
        });
        child.once("exit", () => {
          clearTimeout(timer);
          reject(Error("premature child exit: " + stderr));
        });
      });
      assert.equal(ready.type, "before-provider-fact-commit");
      assert.equal(ready.record.dispatch.certainty, "intent");
      assert.equal(ready.record.state, "dispatching");
      await until(
        () => f.model.requests.length === 1,
        "actual native model request",
      );
      const exited = once(child, "exit");
      child.kill("SIGKILL");
      await exited;
      for (const launch of ready.launches)
        await until(() => groupEmpty(launch.rootPid), "old worker group exit");
      const recovered = await openHost(f.path, "open");
      try {
        const current = await until(async () => {
          const record = unwrap(
            await recovered.store.command(
              ready.session.namespace,
              ready.record.command.commandId,
            ),
          );
          return record.dispatch?.observerGeneration !==
            ready.record.dispatch.observerGeneration &&
            ["reconciliation_required", "terminal"].includes(record.state)
            ? record
            : undefined;
        }, "original attempt reconciled");
        assert.equal(
          current.dispatch.attemptId,
          ready.record.dispatch.attemptId,
        );
        assert.equal(
          current.dispatch.originGeneration,
          ready.record.dispatch.originGeneration,
        );
        assert.equal(
          current.dispatch.nativeSessionId,
          ready.record.dispatch.nativeSessionId,
        );
        assert.notEqual(
          current.outcome,
          "completed",
          "an interrupted held model request cannot become success",
        );
        assert.deepEqual(
          unwrap(
            await recovered.host.submit(
              f.config.caller,
              ready.record.command,
              budget(),
            ),
          ),
          ready.record.receipt,
        );
        assert.equal(f.model.requests.length, 1);
        const session = unwrap(
          await recovered.store.session(ready.session.namespace),
        );
        evidence(
          t,
          "provider-received-host-fact-lost",
          session,
          f.model.requests,
          {
            result: "supported",
            reconciliation: current.state,
            outcome: current.outcome ?? "unknown",
            attempts: 1,
          },
        );
      } finally {
        unwrap(await recovered.host.close(budget()));
      }
      assert.equal(
        f.model.requests.length,
        1,
        "closing recovery must not start deferred duplicate work",
      );
    },
  );

  test(
    `${provider}: Host SQLite restart restores display independently of native context`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      f.model.text("remembered");
      f.model.text("continued");
      await f.start();
      let peer = await f.connect();
      const view = await peer.client.createSession(),
        id = view.namespace.sessionId;
      const input = command(id, "original", "Remember A06_SESSION_NONCE");
      const receipt = await peer.client.submit(input);
      await until(
        () =>
          peer.client.getSession(id)?.commands.original?.state === "terminal",
        "first terminal",
      );
      const before = peer.client.getSession(id);
      const previous = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      ).session;
      await f.stop();
      await f.start();
      peer = await f.connect();
      const restored = await peer.client.restore(id, 1);
      assert.deepEqual(restored.messages, before.messages);
      assert.deepEqual(restored.timeline, before.timeline);
      assert.equal(restored.commands.original.outcome, "completed");
      assert.ok(restored.cursor >= before.cursor);
      assert.equal(
        restored.generation,
        before.generation,
        "display-only restore does not start native work",
      );
      assert.deepEqual(await peer.client.submit(input), receipt);
      assert.equal(
        f.model.requests.length,
        1,
        "display/native recovery does not send a prompt",
      );
      const resumed = await peer.client.resume(id);
      assert.notEqual(resumed.generation, before.generation);
      assert.equal(
        f.model.requests.length,
        1,
        "explicit native resume does not send a prompt",
      );
      await peer.client.submit(
        command(id, "continued", "What did I ask you to remember?"),
      );
      await until(
        () =>
          peer.client.getSession(id)?.commands.continued?.state === "terminal",
        "continued terminal",
      );
      assert.equal(f.model.requests.length, 2);
      assert.match(
        JSON.stringify(f.model.requests[1]),
        /A06_SESSION_NONCE/,
        "native context comes from native history",
      );
      const current = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      ).session;
      assert.equal(
        current.binding.nativeSessionId,
        previous.binding.nativeSessionId,
      );
      if (provider === "codex")
        assert.equal(
          current.binding.nativeThreadId,
          previous.binding.nativeThreadId,
        );
      evidence(
        t,
        "host-restart-display-native-context",
        current,
        f.model.requests,
        {
          result: "supported",
        },
      );
    },
  );
}
