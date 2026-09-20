import assert from "node:assert/strict";
import test from "node:test";
import { assess } from "./check-ai-acceptance.mjs";

test("acceptance cannot pass on absent providers, skipped tests, failed attempts or changed source", () => {
  const rows = ["codex", "claude", "deepseek"].flatMap((provider) =>
    [
      "native-basic-ledger-queue-replay",
      "native-cancel-request-terminal",
      "production-controlled-admission",
      "provider-received-host-fact-lost",
      "host-restart-display-native-context",
      ...(provider === "codex" ? [] : ["question-answer-race-lost-callback"]),
      ...(provider === "deepseek" ? ["cancel-without-terminal-recovery"] : []),
    ].map((scenario) => ({
      provider,
      scenario,
      result:
        scenario === "production-controlled-admission" && provider !== "codex"
          ? "unsupported"
          : "supported",
    })),
  );
  const tests = rows.map(() => ({ status: "pass" }));
  assert.equal(assess(rows, tests, true).passed, true);
  for (const evidence of [
    [],
    rows.slice(1),
    [...rows, rows[0]],
    rows.map((r) => ({ ...r, provider: "codex" })),
  ])
    assert.equal(assess(evidence, tests, true).passed, false);
  assert.equal(assess(rows, tests, false).passed, false);
  assert.equal(assess(rows, tests.slice(1), true).passed, false);
  assert.equal(
    assess(rows, [...tests.slice(1), { status: "fail" }], true).passed,
    false,
  );
  assert.equal(
    assess(
      rows.map((r) => ({ ...r, result: "unknown" })),
      tests,
      true,
    ).passed,
    false,
  );
});
