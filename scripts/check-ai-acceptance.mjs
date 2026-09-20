import { run } from "node:test";
import { createHash } from "node:crypto";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { sourceState, sameCommittedSource } from "./source-state.mjs";

const engines = ["codex", "claude", "deepseek"];
const required = engines.flatMap((provider) =>
  [
    "native-basic-ledger-queue-replay",
    "native-cancel-request-terminal",
    "production-controlled-admission",
    "provider-received-host-fact-lost",
    "host-restart-display-native-context",
    ...(provider === "codex" ? [] : ["question-answer-race-lost-callback"]),
    ...(provider === "deepseek" ? ["cancel-without-terminal-recovery"] : []),
  ].map((scenario) => `${provider}:${scenario}`),
);

export function assess(rows, tests, sourceVerified) {
  const keys = rows.map((row) => `${row.provider}:${row.scenario}`);
  const complete =
    keys.length === required.length &&
    new Set(keys).size === required.length &&
    required.every((key) => keys.includes(key));
  return {
    complete,
    passed:
      complete &&
      sourceVerified &&
      tests.length === required.length &&
      tests.every((t) => t.status === "pass") &&
      rows.every(
        (r) =>
          r.result ===
          (r.scenario === "production-controlled-admission" &&
          r.provider !== "codex"
            ? "unsupported"
            : "supported"),
      ),
    missing: required.filter((key) => !keys.includes(key)),
  };
}

async function main() {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const start = sourceState(root);
  const hash = (name) =>
    createHash("sha256")
      .update(readFileSync(new URL("../" + name, import.meta.url)))
      .digest("hex");
  const locks = () => ({
    pnpm: hash("pnpm-lock.yaml"),
    cargo: hash("Cargo.lock"),
  });
  const before = locks(),
    rows = [],
    tests = [];
  const files = [
    "tests/ai-provider-conformance/native-host.test.mjs",
    "tests/ai-provider-conformance/cancellation.test.mjs",
    "tests/ai-provider-conformance/controlled.test.mjs",
    "tests/ai-provider-conformance/questions.test.mjs",
    "tests/ai-recovery-integration/native-recovery.test.mjs",
  ].map((file) => fileURLToPath(new URL("../" + file, import.meta.url)));
  for await (const event of run({ files, concurrency: 1, timeout: 120000 })) {
    if (
      event.type === "test:diagnostic" &&
      event.data.message.startsWith('{"a06":1,')
    ) {
      rows.push(JSON.parse(event.data.message));
    } else if (["test:pass", "test:fail"].includes(event.type)) {
      const status =
        event.type === "test:pass" && !event.data.skip && !event.data.todo
          ? "pass"
          : "fail";
      tests.push({ name: event.data.name, status });
      console.log(`[a06] ${status.toUpperCase()} ${event.data.name}`);
      if (status === "fail") console.error(event.data.details?.error);
    } else if (event.type === "test:stderr")
      process.stderr.write(event.data.message);
  }
  const end = sourceState(root),
    after = locks();
  const sourceVerified =
    sameCommittedSource(start, end) &&
    JSON.stringify(before) === JSON.stringify(after);
  const verdict = assess(rows, tests, sourceVerified);
  const receipt = {
    schemaVersion: 1,
    command: "pnpm test:ai-acceptance",
    timestamp: new Date().toISOString(),
    source: { start, end },
    lockSha256: { before, after },
    sourceVerified,
    runtime: {
      platform: process.platform,
      arch: process.arch,
      node: process.version,
    },
    configuration: {
      modelEndpoint: "loopback_protocol_fixture",
      credentials: "fixture_only",
      nativePlugins: "fixed adapter profiles from the recorded source and lock",
    },
    verdict,
    tests,
    rows,
    notVerified: [
      "external real models (not run by this command)",
      "Windows",
      "OS T3",
      "same-UID arbitrary filesystem/network/IPC containment",
      "enterprise identity and real executor",
    ],
  };
  mkdirSync(new URL("../.local-ci-runs/", import.meta.url), {
    recursive: true,
  });
  writeFileSync(
    new URL("../.local-ci-runs/ai-provider-matrix.json", import.meta.url),
    JSON.stringify(receipt, null, 2) + "\n",
  );
  console.log("[a06] receipt: .local-ci-runs/ai-provider-matrix.json", verdict);
  process.exitCode = verdict.passed ? 0 : 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
