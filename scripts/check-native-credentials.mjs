import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { once } from "node:events";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { run, verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
import { sameCommittedSource, sourceState } from "./source-state.mjs";

const repository = fileURLToPath(new URL("../", import.meta.url));
const artifact = join(repository, ".local-ci-runs/ai-host-runtime");
const output = join(repository, ".local-ci-runs/native-credentials.json");
const start = sourceState(repository);
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("acceptance requires macOS arm64");
if (!start.clean) throw new Error("committed_source_required");
const manifestBytes = readFileSync(join(artifact, "manifest.json"));
const manifest = JSON.parse(manifestBytes);
if (
  manifest.status !== "passed" ||
  !sameCommittedSource(start, manifest.source.end)
)
  throw new Error("same-source fixed artifact required");
verifyRuntimeIntegrity(artifact, manifest.runtimeTreeSha256);

const directory = realpathSync(
  mkdtempSync(join(tmpdir(), "rss-native-credentials-")),
);
chmodSync(directory, 0o700);
const resultPath = join(directory, "result.json");
const secretPath = join(directory, "fixture-secret");
const syntheticSecret = `rss-local-fixture-${randomBytes(24).toString("base64url")}`;
const expectedAuthorization = `Bearer ${syntheticSecret}`;
const model = "local-deepseek-protocol-fixture";
let requests = 0;
let authenticatedRequests = 0;
let validProbeBodies = 0;
let outputLeak = false;
let behavior;
let exit;
let failure;
let ciphertextCleared = false;
let executableSha256;

const fixture = createServer((request, response) => {
  const chunks = [];
  request.on("data", (chunk) => chunks.push(chunk));
  request.on("end", () => {
    requests += 1;
    let validBody = false;
    try {
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      validBody =
        request.method === "POST" &&
        request.url === "/v1/chat/completions" &&
        body.model === model &&
        body.stream === true &&
        Array.isArray(body.messages) &&
        JSON.stringify(body.messages).includes("Reply with OK only");
    } catch {}
    if (request.headers.authorization !== expectedAuthorization || !validBody) {
      response.writeHead(401, { "content-type": "application/json" });
      response.end('{"error":{"type":"authentication_error"}}');
      return;
    }
    authenticatedRequests += 1;
    validProbeBodies += 1;
    response.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      "x-request-id": "local-protocol-fixture",
    });
    const common = {
      id: "local-protocol-fixture",
      object: "chat.completion.chunk",
      created: 1,
      model,
    };
    response.write(
      `data: ${JSON.stringify({ ...common, choices: [{ index: 0, delta: { role: "assistant", content: "OK" }, finish_reason: null }] })}\n\n`,
    );
    response.write(
      `data: ${JSON.stringify({ ...common, choices: [{ index: 0, delta: {}, finish_reason: "stop" }], usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } })}\n\n`,
    );
    response.end("data: [DONE]\n\n");
  });
});
fixture.listen(0, "127.0.0.1");
await once(fixture, "listening");
const address = fixture.address();
assert.ok(address && typeof address === "object");

try {
  writeFileSync(secretPath, syntheticSecret, { mode: 0o600 });
  writeFileSync(
    join(directory, "acceptance.json"),
    JSON.stringify({
      apiUrl: `http://127.0.0.1:${address.port}/v1`,
      model,
    }),
    { mode: 0o600 },
  );
  run("pnpm", ["build"], repository);
  run(
    "cargo",
    [
      "build",
      "--locked",
      "-p",
      "rss-mdm-desktop",
      "--example",
      "custom-connection-acceptance",
    ],
    repository,
  );
  const executable = join(
    repository,
    "target/debug/examples/custom-connection-acceptance",
  );
  executableSha256 = sha256(readFileSync(executable));
  await new Promise((resolve, reject) => {
    const child = spawn(executable, [directory, artifact, resultPath], {
      cwd: repository,
      stdio: ["ignore", "pipe", "pipe"],
    });
    for (const stream of [child.stdout, child.stderr]) {
      let tail = "";
      stream.on("data", (chunk) => {
        const text = tail + chunk.toString();
        outputLeak ||= text.includes(syntheticSecret);
        tail = text.slice(-syntheticSecret.length);
      });
    }
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("native process timeout"));
    }, 210000);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      exit = { code, signal };
      resolve();
    });
  });
  behavior = existsSync(resultPath)
    ? JSON.parse(readFileSync(resultPath, "utf8"))
    : { step: "failed", stage: "native_exit_without_report" };
  if (exit.code !== 0 || exit.signal !== null)
    throw new Error(`native_process_failed_at_${behavior.stage ?? "unknown"}`);
  assert.equal(behavior.step, "passed");
  assert.equal(behavior.secureEntry, true);
  assert.equal(behavior.secretOutputsClean, true);
  assert.equal(outputLeak, false);
  for (const file of [resultPath, join(directory, "diagnostics.json")])
    assert.equal(
      readFileSync(file).includes(Buffer.from(syntheticSecret)),
      false,
    );
  assert.equal(behavior.deleted, true);
  assert.equal(behavior.modelProbe, "local_openai_compatible_protocol");
  assert.equal(requests, 1);
  assert.equal(authenticatedRequests, 1);
  assert.equal(validProbeBodies, 1);

  const database = new DatabaseSync(join(directory, "ai.sqlite"), {
    readOnly: true,
  });
  let revisions;
  try {
    revisions = database
      .prepare(
        "SELECT principal_id,json,encrypted_secret FROM connections ORDER BY revision",
      )
      .all()
      .map((row) => ({
        principalId: row.principal_id,
        encrypted: row.encrypted_secret,
        value: JSON.parse(row.json),
      }));
  } finally {
    database.close();
  }
  assert.equal(revisions.length, 2);
  assert.equal(revisions[0].value.provider, "deepseek");
  assert.equal(revisions[0].value.source.type, "custom_api");
  assert.equal(revisions[0].value.status, "ready");
  assert.equal(revisions[1].value.status, "deleted");
  ciphertextCleared = revisions.every((row) => row.encrypted === null);
  assert.equal(ciphertextCleared, true);
  assert.equal(
    readFileSync(join(directory, "ai.sqlite")).includes(
      Buffer.from(syntheticSecret),
    ),
    false,
  );
} catch (error) {
  failure = error instanceof Error ? error.message : "acceptance_failed";
  process.exitCode = 1;
} finally {
  await new Promise((resolve) => fixture.close(resolve));
  const end = sourceState(repository);
  const passed =
    !failure &&
    behavior?.step === "passed" &&
    authenticatedRequests === 1 &&
    validProbeBodies === 1 &&
    ciphertextCleared &&
    !outputLeak &&
    behavior?.secretOutputsClean === true &&
    sameCommittedSource(start, end);
  mkdirSync(join(repository, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    output,
    JSON.stringify(
      {
        status: passed ? "passed" : "failed",
        command: "node scripts/check-native-credentials.mjs",
        source: { start, end, unchanged: sameCommittedSource(start, end) },
        locks: {
          pnpmSha256: sha256(readFileSync(join(repository, "pnpm-lock.yaml"))),
          cargoSha256: sha256(readFileSync(join(repository, "Cargo.lock"))),
        },
        artifact: {
          runtimeManifestSha256: sha256(manifestBytes),
          runtimeTreeSha256: manifest.runtimeTreeSha256,
          nativeExampleSha256: executableSha256,
        },
        runtime: {
          node: process.versions.node,
          platform: process.platform,
          arch: process.arch,
        },
        mode: "production AppKit secure entry/private channel/Host/encrypted SQLite; injected test master key; local OpenAI-compatible protocol",
        cloudAuthentication: false,
        syntheticCredential: true,
        secretOutputsClean:
          !outputLeak && behavior?.secretOutputsClean === true,
        fixture: {
          requests,
          authenticatedRequests,
          validProbeBodies,
          responseProtocol: "OpenAI-compatible SSE",
        },
        cleanup: { ciphertextCleared },
        keychainAccess: false,
        behavior,
        exit,
        failure,
      },
      null,
      2,
    ),
  );
  rmSync(directory, { recursive: true, force: true });
  if (!passed) process.exitCode = 1;
}
