import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
// Real macOS WebView acceptance; uses the fixed runtime artifact and existing user login.
import { spawn } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
import { run, verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root);
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("acceptance requires macOS arm64");
const artifact = join(root, ".local-ci-runs/ai-host-runtime"),
  manifest = JSON.parse(readFileSync(join(artifact, "manifest.json"), "utf8"));
if (
  manifest.status !== "passed" ||
  !sameCommittedSource(start, manifest.source.end)
)
  throw new Error("same-source fixed artifact required");
verifyRuntimeIntegrity(artifact, manifest.runtimeTreeSha256);
const directory = realpathSync(mkdtempSync(join(tmpdir(), "rss-desktop-"))),
  report = join(directory, "result.json");
const model = process.env.CODEX_SMOKE_MODEL ?? "gpt-5.5";
writeFileSync(
  join(directory, "client.json"),
  JSON.stringify({
    databasePath: join(directory, "ai.sqlite"),
    socketPath: join(directory, "ai.sock"),
    nativeDirectory: join(directory, "native"),
    workingDirectory: join(directory, "workspace"),
    caller: {
      tenantId: "s1-test",
      principalId: "fixture-actor",
      authorityId: "desktop-fixture",
    },
    session: {
      provider: "codex",
      accountRef: "s1-user-codex",
      config: { id: "s1-local", revision: "r1" },
      profile: "controlled_tools",
    },
    connection: {
      source: "existing_user_config",
      directory: process.env.CODEX_HOME ?? join(homedir(), ".codex"),
      model,
    },
  }),
  { mode: 0o600 },
);
let behavior, failure, facts, exit;
try {
  run("pnpm", ["build"], root);
  run(
    "cargo",
    [
      "build",
      "-p",
      "rss-mdm-desktop",
      "--example",
      "desktop-acceptance",
      "--locked",
    ],
    root,
  );
  await new Promise((resolve, reject) => {
    const child = spawn(
      join(root, "target/debug/examples/desktop-acceptance"),
      [directory, artifact, report],
      { cwd: root, stdio: ["ignore", "ignore", "inherit"] },
    );
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("native process timeout"));
    }, 270000);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      exit = { code, signal };
      if (code !== 0 || signal !== null)
        reject(new Error("native process did not exit cleanly"));
      else resolve();
    });
  });
  behavior = JSON.parse(readFileSync(report, "utf8"));
  if (behavior.step !== "passed")
    throw new Error(`native acceptance failed: ${behavior.stage}`);
  const ai = new DatabaseSync(join(directory, "ai.sqlite"), { readOnly: true });
  const execution = new DatabaseSync(join(directory, "execution.sqlite"), {
    readOnly: true,
  });
  try {
    const sessions = ai
      .prepare("SELECT json FROM sessions")
      .all()
      .map((row) => JSON.parse(row.json));
    assert.equal(sessions.length, 1);
    const session = sessions[0];
    assert.equal(session.binding.provider, "codex");
    assert.equal(session.binding.providerVersion, "0.155.0");
    assert.equal(session.capabilities.tools, "host_mediated");
    assert.equal(
      ai.prepare("SELECT count(*) n FROM worker_launches").get().n,
      0,
    );
    const commands = ai
      .prepare("SELECT json FROM commands")
      .all()
      .map((row) => JSON.parse(row.json));
    assert.equal(commands.length, 1);
    assert.equal(commands[0].state, "terminal");
    assert.equal(commands[0].outcome, "completed");
    const deliveries = ai
      .prepare("SELECT status,count(*) n FROM deliveries GROUP BY status")
      .all();
    assert.ok(
      deliveries.length === 1 &&
        deliveries[0].status === "delivered" &&
        deliveries[0].n >= 5,
    );
    const tasks = execution
      .prepare(
        "SELECT request_id,digest,plan,snapshot FROM executions ORDER BY request_id",
      )
      .all()
      .map((row) => {
        const plan = JSON.parse(Buffer.from(row.plan).toString()),
          state = JSON.parse(Buffer.from(row.snapshot).toString());
        assert.equal(state.attempts, 1);
        assert.equal(state.attempt.mode, "test");
        assert.equal(
          state.attempt.assessment.observation.assessment,
          "satisfied",
        );
        assert.equal(state.attempt.termination.observation.exitCode, 0);
        assert.equal(
          plan.request.initiator.kind,
          row.request_id.startsWith("ai-s1-") ? "ai" : "human",
        );
        return {
          requestId: row.request_id,
          planId: plan.planId,
          digest: row.digest,
          origin: plan.request.initiator.kind,
          attempts: state.attempts,
          cancelRequested: state.cancelRequested,
          evidenceKind: state.attempt.assessment.evidence.kind,
        };
      });
    assert.equal(tasks.length, 3);
    facts = {
      sessionId: session.namespace.sessionId,
      binding: session.binding,
      tasks,
      deliveries,
      workerLaunchesRemaining: 0,
    };
  } finally {
    ai.close();
    execution.close();
  }
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  const end = sourceState(root),
    passed =
      !failure &&
      Boolean(facts) &&
      behavior?.step === "passed" &&
      sameCommittedSource(start, end);
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/desktop-native.json"),
    JSON.stringify(
      {
        status: passed ? "passed" : "failed",
        source: { start, end },
        platform: process.platform,
        arch: process.arch,
        lockSha256: createHash("sha256")
          .update(readFileSync(join(root, "pnpm-lock.yaml")))
          .digest("hex"),
        runtimeManifestSha256: createHash("sha256")
          .update(readFileSync(join(artifact, "manifest.json")))
          .digest("hex"),
        authentication: "existing_user_config",
        model,
        modelFixture: false,
        executor: "S1 deterministic test runner",
        behavior,
        facts,
        exit,
        failure,
      },
      null,
      2,
    ),
  );
  rmSync(directory, { recursive: true, force: true });
  if (!passed) process.exitCode = 1;
  if (failure) console.error(failure);
}
