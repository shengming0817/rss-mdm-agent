import assert from "node:assert/strict";
import test from "node:test";
import { createRequire } from "node:module";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  assemble,
  COMPOSITION_ID,
} from "../../../packages/ai-adapters/deepseek/dist/assembly.js";
import { sealTools } from "../../../packages/ai-adapters/deepseek/dist/guard.js";
const require = createRequire(
  new URL(
    "../../../packages/ai-adapters/deepseek/package.json",
    import.meta.url,
  ),
);
const { Context } = require("@deepseek-ai/cordis");
const { ToolCallId } = require("@deepseek-ai/dsh-llm");
const { SessionId } = require("@deepseek-ai/dsh-session");
for (const mode of ["dispose", "update"])
  test(`required non-tool fiber ${mode} invalidates the observed assembly permanently`, async () => {
    const dir = await mkdtemp(join(tmpdir(), "rss-dsh-fibers-")),
      ctx = new Context(),
      oldHome = process.env.DSH_HOME;
    process.env.DSH_HOME = dir;
    try {
      let drifts = 0;
      const observed = await assemble(
        ctx,
        {
          nativeSessionId: "fiber-session",
          workingDirectory: dir,
          persistenceDirectory: dir,
          scope: "fibers",
          model: "deepseek-chat",
          apiKey: "fixture",
          apiUrl: "http://127.0.0.1:1",
          controlled: false,
          restore: false,
          composition: COMPOSITION_ID,
        },
        () => drifts++,
      );
      observed.verify();
      // The checkpoint service is a policy plugin, not a tool inventory change.
      const checkpoint = require("@deepseek-ai/dsh-session-checkpoint-policy");
      const runtime = ctx.registry.get(checkpoint);
      assert.ok(runtime);
      if (mode === "dispose") await [...runtime.fibers][0].dispose();
      else assert.throws(() => [...runtime.fibers][0].update({}));
      assert.equal(drifts, 1);
      assert.throws(observed.verify);
      await ctx.plugin(checkpoint);
      assert.throws(
        observed.verify,
        "a replacement cannot revive old admission",
      );
    } finally {
      await ctx.fiber.dispose();
      if (oldHome === undefined) delete process.env.DSH_HOME;
      else process.env.DSH_HOME = oldHome;
      await rm(dir, { recursive: true, force: true });
    }
  });
test("deny guard wins over later allow, rejects nested/delegated callers and invalidates on inventory drift", async () => {
  const dir = await mkdtemp(join(tmpdir(), "rss-dsh-guard-")),
    ctx = new Context(),
    oldHome = process.env.DSH_HOME;
  process.env.DSH_HOME = dir;
  try {
    await assemble(ctx, {
      nativeSessionId: "guard-session",
      workingDirectory: dir,
      persistenceDirectory: dir,
      scope: "guard",
      model: "deepseek-chat",
      apiKey: "fixture",
      apiUrl: "http://127.0.0.1:1",
      controlled: true,
      restore: false,
      composition: COMPOSITION_ID,
    });
    let calls = 0,
      drifts = 0;
    const definition = {
      name: "host_propose",
      description: "test bridge",
      parameters: {},
      output: { schema: { type: "null" }, render: () => [] },
      execute: async () => {
        calls++;
        return null;
      },
    };
    ctx.tools.register(definition);
    const verify = sealTools(
      ctx,
      "guard-session",
      ["ask_user_question", "host_propose"],
      () => drifts++,
    );
    const handle = await ctx.agents.create({
      sessionId: SessionId("guard-session"),
      agentOptions: { provider: "deepseek-official", model: "deepseek-chat" },
    });
    ctx.on("tools/pre-execute", async () => ({ kind: "allow" }));
    const call = (patch) =>
      ctx.tools.execute({
        callId: ToolCallId("test-call"),
        name: "host_propose",
        arguments: {},
        agent: handle.agent,
        signal: new AbortController().signal,
        ...patch,
      });
    assert.equal((await call({ parent: Symbol("nested") })).isError, true);
    assert.equal((await call({ agent: undefined })).isError, true);
    assert.equal(calls, 0);
    const denied = ctx.tools.guard(() => "deny first");
    assert.equal((await call({})).isError, true);
    assert.equal(calls, 0);
    denied();
    assert.equal((await call({})).isError, false);
    assert.equal(calls, 1);
    ctx.tools.register({ ...definition, name: "shell" });
    assert.equal(drifts, 1);
    assert.throws(verify);
    assert.equal((await call({})).isError, true);
    assert.equal(calls, 1);
  } finally {
    await ctx.fiber.dispose();
    if (oldHome === undefined) delete process.env.DSH_HOME;
    else process.env.DSH_HOME = oldHome;
    await rm(dir, { recursive: true, force: true });
  }
});

test("real checkpoint policy refuses both model and tool effects when flush fails", async () => {
  const dir = await mkdtemp(join(tmpdir(), "rss-dsh-flush-")),
    ctx = new Context(),
    oldHome = process.env.DSH_HOME;
  process.env.DSH_HOME = dir;
  try {
    await assemble(ctx, {
      nativeSessionId: "flush-session",
      workingDirectory: dir,
      persistenceDirectory: dir,
      scope: "flush",
      model: "deepseek-chat",
      apiKey: "fixture",
      apiUrl: "http://127.0.0.1:1",
      controlled: true,
      restore: false,
      composition: COMPOSITION_ID,
    });
    const { LlmAdapter, createUserMessage } = require("@deepseek-ai/dsh-llm");
    let requests = 0,
      effects = 0;
    class Model extends LlmAdapter {
      async *stream() {
        requests++;
        yield { type: "finish", reason: { kind: "stop" } };
      }
    }
    ctx.llm.registerAdapter(["checkpoint-fixture"], new Model());
    ctx.tools.register({
      name: "test_effect",
      description: "effect fixture",
      parameters: {},
      output: { schema: { type: "null" }, render: () => [] },
      execute: async () => {
        effects++;
        return null;
      },
    });
    const handle = await ctx.agents.create({
      sessionId: SessionId("flush-session"),
      agentOptions: { provider: "checkpoint-fixture", model: "test" },
    });
    ctx.sessions.flush = async () => {
      throw Error("fixture flush failure");
    };
    const result = await ctx.tools.execute({
      callId: ToolCallId("flush-tool"),
      name: "test_effect",
      arguments: {},
      agent: handle.agent,
      signal: new AbortController().signal,
    });
    assert.equal(result.isError, true);
    assert.equal(effects, 0);
    handle.agent.followup(
      createUserMessage({
        content: [{ type: "text", text: "test flush" }],
        source: { kind: "user" },
      }),
    );
    await handle.agent.whenIdle();
    assert.equal(requests, 0);
  } finally {
    await ctx.fiber.dispose();
    if (oldHome === undefined) delete process.env.DSH_HOME;
    else process.env.DSH_HOME = oldHome;
    await rm(dir, { recursive: true, force: true });
  }
});
