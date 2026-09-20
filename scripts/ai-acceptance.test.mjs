import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { assess, capabilities, required } from "./check-ai-acceptance.mjs";

function fixture() {
  const installations = Object.fromEntries(
    ["codex", "claude", "deepseek"].map((provider) => [
      provider,
      {
        providerVersion: "native-" + provider,
        adapterVersion: "adapter-1",
        profileSourceSha256: "a".repeat(64),
      },
    ]),
  );
  const rows = required.map((key, index) => {
    const [provider, scenario] = key.split(":"),
      controlled = scenario === "production-controlled-admission";
    const rejected = controlled && provider !== "codex";
    return {
      a06: 1,
      provider,
      scenario,
      profileSourceSha256: installations[provider].profileSourceSha256,
      profile: controlled ? "controlled_tools" : "conversation",
      proof: rejected
        ? "production_admission"
        : controlled
          ? "real_process_local_model_rust_s1"
          : "real_process_local_model",
      result:
        rejected ||
        (scenario === "native-steer-difference" && provider !== "codex")
          ? "unsupported"
          : "supported",
      ...(rejected
        ? { modelRequests: 0 }
        : {
            binding: {
              provider,
              providerVersion: installations[provider].providerVersion,
              adapterVersion: "adapter-1",
              config: { id: "local", revision: "r1" },
              accountRef: "test-account",
              generation: "generation-" + index,
              nativeSessionId: "session-" + index,
              ...(provider === "codex"
                ? { nativeThreadId: "thread-" + index }
                : {}),
            },
            capabilities: capabilities(
              provider,
              controlled ? "host_mediated" : "disabled",
            ),
          }),
    };
  });
  return {
    rows,
    tests: rows.map(() => ({ status: "pass" })),
    context: {
      sourceVerified: true,
      installations,
      runtime: {
        platform: "darwin",
        arch: "arm64",
        node:
          "v" +
          JSON.parse(readFileSync(new URL("../package.json", import.meta.url)))
            .engines.node,
      },
    },
  };
}

test("acceptance rejects missing scenarios, test failures, unsupported platforms and changed source", () => {
  const { rows, tests, context } = fixture();
  assert.equal(assess(rows, tests, context).passed, true);
  for (const evidence of [
    [],
    rows.slice(1),
    [...rows, rows[0]],
    rows.map((r) => ({ ...r, provider: "codex" })),
  ])
    assert.equal(assess(evidence, tests, context).passed, false);
  assert.equal(
    assess(rows, tests, { ...context, sourceVerified: false }).passed,
    false,
  );
  for (const [key, value] of [
    ["platform", "win32"],
    ["arch", "x64"],
    ["node", "v0"],
  ])
    assert.equal(
      assess(rows, tests, {
        ...context,
        runtime: { ...context.runtime, [key]: value },
      }).passed,
      false,
    );
  assert.equal(assess(rows, tests.slice(1), context).passed, false);
  assert.equal(
    assess(rows, [...tests.slice(1), { status: "fail" }], context).passed,
    false,
  );
});

test("evidence cannot omit or misbind its native/profile/configuration identity", () => {
  const { rows, tests, context } = fixture();
  for (const field of [
    "a06",
    "profile",
    "proof",
    "profileSourceSha256",
    "capabilities",
    "result",
    "binding",
  ])
    for (const value of [undefined, "wrong"])
      assert.equal(
        assess(
          [{ ...rows[0], [field]: value }, ...rows.slice(1)],
          tests,
          context,
        ).passed,
        false,
        field,
      );
  for (const field of [
    "provider",
    "providerVersion",
    "adapterVersion",
    "config",
    "accountRef",
    "generation",
    "nativeSessionId",
    "nativeThreadId",
  ])
    assert.equal(
      assess(
        [
          { ...rows[0], binding: { ...rows[0].binding, [field]: undefined } },
          ...rows.slice(1),
        ],
        tests,
        context,
      ).passed,
      false,
      field,
    );
  for (const patch of [
    { provider: "claude" },
    { providerVersion: "wrong" },
    { adapterVersion: "wrong" },
    { config: { id: "local", revision: "r2" } },
    { generation: rows[1].binding.generation },
  ])
    assert.equal(
      assess(
        [
          { ...rows[0], binding: { ...rows[0].binding, ...patch } },
          ...rows.slice(1),
        ],
        tests,
        context,
      ).passed,
      false,
    );
  const rejected = rows.findIndex((r) => r.proof === "production_admission");
  for (const patch of [
    { modelRequests: 1 },
    { binding: rows[0].binding },
    { result: "supported" },
  ]) {
    const changed = rows.map((r, i) =>
      i === rejected ? { ...r, ...patch } : r,
    );
    assert.equal(assess(changed, tests, context).passed, false);
  }
  assert.equal(
    assess(
      rows.map(({ provider, scenario, result }) => ({
        provider,
        scenario,
        result,
      })),
      tests,
      context,
    ).passed,
    false,
  );
});
