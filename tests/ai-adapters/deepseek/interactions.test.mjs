import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  fixtureAttempt,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { budget } from "./support.mjs";
import {
  environment,
  completion,
  command,
  collect,
  files,
} from "./native-support.mjs";
function tool(res, name, args) {
  res.writeHead(200, { "content-type": "text/event-stream" });
  const chunk = {
    id: "tool-request",
    object: "chat.completion.chunk",
    choices: [
      {
        index: 0,
        delta: {
          tool_calls: [
            {
              index: 0,
              id: "tool-call-1",
              type: "function",
              function: { name, arguments: JSON.stringify(args) },
            },
          ],
        },
        finish_reason: null,
      },
    ],
  };
  res.write(`data: ${JSON.stringify(chunk)}\n\n`);
  res.end(
    `data: ${JSON.stringify({ id: "tool-request", choices: [{ index: 0, delta: {}, finish_reason: "tool_calls" }] })}\n\ndata: [DONE]\n\n`,
  );
}
test("native structured question is generation bound; answer only returns a tool result", async (t) => {
  const env = await environment(t, (_b, res, n) =>
    n === 1
      ? tool(res, "ask_user_question", {
          questions: [
            {
              id: "q",
              question: "Choose?",
              options: [{ label: "yes" }, { label: "no" }],
            },
          ],
        })
      : completion(res),
  );
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(p, env.config, budget()),
    ),
    c = command(),
    a = fixtureAttempt(admitted.binding, c),
    sent = await p.submit(admitted.binding, c, a, budget());
  assert.equal(sent.certainty, "submitted");
  let seen = false;
  const observations = [];
  for await (const e of p.observe(sent.binding, budget(10000))) {
    observations.push(e);
    if (e.type === "interaction") {
      seen = true;
      assert.equal(e.interaction.callbackLifetime, "generation_bound");
      const response = {
        ...command("answer"),
        input: {
          type: "respond",
          interactionId: e.interaction.interactionId,
          generation: e.binding.generation,
          answer: { answers: [{ id: "q", selected: ["yes"] }] },
        },
      };
      assert.equal(
        (
          await p.respond(
            { ...e.binding, generation: "old" },
            response,
            budget(),
          )
        ).ok,
        false,
      );
      unwrap(await p.respond(e.binding, response, budget()));
    }
  }
  assert.equal(seen, true, JSON.stringify(observations));
  assert.equal(observations.at(-1)?.body?.outcome, "completed");
  assert.ok(
    env.requests[1].messages.some(
      (m) => m.role === "tool" && JSON.stringify(m.content).includes("yes"),
    ),
  );
});
test("controlled profile has only question + proposal and checkpoints before host effects", async (t) => {
  let proposals = 0,
    env;
  env = await environment(
    t,
    (_b, res, n) =>
      n === 1
        ? tool(res, "host_propose", {
            name: "test_operation",
            arguments: { value: 7 },
          })
        : completion(res),
    (config) => {
      config.permissions = "host_mediated";
      config.verifier = {
        verify: async () => ({
          ok: true,
          value: {
            platform: process.platform,
            verificationRef: "fixture-platform-proof",
          },
        }),
      };
      config.tools = {
        propose: async (proposal) => {
          proposals++;
          assert.equal(proposal.name, "test_operation");
          assert.match(
            Object.values(await files(env.dir)).join("\n"),
            /tool\/call/,
          );
          return {
            ok: true,
            value: { disposition: "returned", text: "S1 fixture only" },
          };
        },
      };
    },
  );
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(p, env.config, budget()),
    ),
    c = command(),
    sent = await p.submit(
      admitted.binding,
      c,
      fixtureAttempt(admitted.binding, c),
      budget(),
    );
  const events = await collect(p, sent.binding);
  assert.equal(events.at(-1)?.body?.outcome, "completed");
  assert.equal(proposals, 1);
  assert.deepEqual(env.requests[0].tools.map((t) => t.function.name).sort(), [
    "ask_user_question",
    "host_propose",
  ]);
  assert.ok(events.some((e) => e.body?.type === "tool_proposal"));
  assert.ok(events.some((e) => e.body?.type === "tool_result"));
});
for (const name of [
  "shell",
  "terminal",
  "run_code",
  "delegate",
  "mcp__evil",
  "host_propose",
])
  test(`conversation native bypass refused: ${name}`, async (t) => {
    const env = await environment(t, (_b, res, n) =>
      n === 1
        ? tool(res, name, { command: "echo forbidden" })
        : completion(res),
    );
    const p = env.port(),
      admitted = unwrap(
        await VerifiedProviderSession.open(p, env.config, budget()),
      ),
      c = command(),
      sent = await p.submit(
        admitted.binding,
        c,
        fixtureAttempt(admitted.binding, c),
        budget(),
      );
    const events = await collect(p, sent.binding);
    assert.equal(events.at(-1)?.body?.outcome, "completed");
    assert.deepEqual(
      env.requests[0].tools.map((t) => t.function.name),
      ["ask_user_question"],
    );
    assert.ok(
      env.requests[1].messages.some(
        (m) =>
          m.role === "tool" &&
          /Error|not found|Unknown/i.test(JSON.stringify(m.content)),
      ),
    );
    assert.ok(!events.some((e) => e.body?.type === "tool_proposal"));
  });
test("native cancellation invalidates callbacks without fabricating terminal on transport loss", async (t) => {
  const env = await environment(t, (_b, res, n) =>
    n === 1
      ? tool(res, "ask_user_question", {
          questions: [{ id: "q", question: "Waiting?" }],
        })
      : completion(res),
  );
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(p, env.config, budget()),
    ),
    c = command(),
    sent = await p.submit(
      admitted.binding,
      c,
      fixtureAttempt(admitted.binding, c),
      budget(),
    );
  const observations = [];
  for await (const e of p.observe(sent.binding, budget(10000))) {
    observations.push(e);
    if (e.type === "interaction") {
      const cancel = {
        ...command("cancel"),
        input: {
          type: "cancel",
          targetCommandId: c.commandId,
          generation: e.binding.generation,
        },
      };
      assert.equal(
        unwrap(await p.cancel(e.binding, cancel, budget())),
        "request_only",
      );
    }
  }
  assert.ok(observations.some((e) => e.type === "interaction_unavailable"));
  assert.equal(observations.at(-1)?.body?.outcome, "cancelled");
});

test("provider HTTP error has sanitized error and native failed terminal", async (t) => {
  const env = await environment(t, (_b, res) => {
    res.writeHead(401, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        error: {
          message: "secret-upstream-debug-value",
          type: "authentication_error",
        },
      }),
    );
  });
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(p, env.config, budget()),
    ),
    c = command(),
    sent = await p.submit(
      admitted.binding,
      c,
      fixtureAttempt(admitted.binding, c),
      budget(),
    );
  const events = await collect(p, sent.binding);
  assert.equal(events.at(-1)?.body?.outcome, "failed");
  assert.ok(events.some((e) => e.body?.type === "error"));
  assert.ok(!JSON.stringify(events).includes("secret-upstream"));
});
