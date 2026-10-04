import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  Preparation,
  contextKey,
  preparedArtifact,
  sourceDigest,
} from "./preparation.mjs";
import { executeSteps, runCommand } from "./ci-plan.mjs";

function fixture(t, failed = false) {
  const root = mkdtempSync(join(tmpdir(), "agent-preparation-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "crates/sample"), { recursive: true });
  writeFileSync(join(root, "crates/sample/input.rs"), "// baseline\n");
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({
      engines: { node: process.versions.node },
      packageManager: "pnpm@11.4.0",
    }),
  );
  const env = { ...process.env };
  for (const key of [
    contextKey,
    "_AGENT_BUILD_LEASE",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
  ])
    delete env[key];
  const calls = [];
  const preparation = new Preparation(root, {
    env,
    execute: async (command, args) => {
      calls.push([command, ...args]);
      if (failed) return { status: 1 };
      const example = args[args.indexOf("--example") + 1];
      const executable = join(root, "compiler-output");
      writeFileSync(executable, `executable: ${example}`);
      return {
        status: 0,
        stdout:
          JSON.stringify({
            reason: "compiler-artifact",
            target: { name: example, kind: ["example"] },
            executable,
          }) + "\n",
      };
    },
  });
  t.after(() => preparation.close());
  return { root, preparation, calls };
}

test("one Cargo preparation serves independent Node processes and fixture restarts", async (t) => {
  const { root, preparation, calls } = fixture(t);
  await preparation.ensure(["execution"]);
  const executable = preparedArtifact("execution", root, preparation.env);
  const module = new URL("./preparation.mjs", import.meta.url).href;
  for (let index = 0; index < 2; index++) {
    const output = execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `import {preparedArtifact} from ${JSON.stringify(module)};console.log(preparedArtifact('execution',process.argv[1]));console.log(preparedArtifact('execution',process.argv[1]));`,
        root,
      ],
      { env: preparation.env, encoding: "utf8" },
    );
    assert.deepEqual(output.trim().split("\n"), [executable, executable]);
  }
  await preparation.ensure(["execution"]);
  assert.equal(calls.length, 1);
  assert.ok(calls[0].includes("test-support"));
  // Cargo can overwrite its original target later; the run's copied executable remains stable.
  writeFileSync(join(root, "compiler-output"), "another Cargo configuration");
  assert.equal(
    preparedArtifact("execution", root, preparation.env),
    executable,
  );
});

test("missing/damaged binaries, different configuration and missing features cannot reuse preparation", async (t) => {
  const { root, preparation } = fixture(t);
  await preparation.ensure(["execution"]);
  assert.throws(
    () => preparedArtifact("boundaries", root, preparation.env),
    /missing or changed/,
  );
  assert.throws(
    () =>
      preparedArtifact("execution", root, {
        ...preparation.env,
        CARGO_PROFILE_DEV_DEBUG: "0",
      }),
    /inputs changed/,
  );
  const path = preparedArtifact("execution", root, preparation.env);
  writeFileSync(path, "tampered");
  assert.throws(
    () => preparedArtifact("execution", root, preparation.env),
    /missing or changed/,
  );
  rmSync(path);
  assert.throws(
    () => preparedArtifact("execution", root, preparation.env),
    /missing or changed/,
  );
});

test("dirty source bytes and expired run contexts never reuse a success", async (t) => {
  const { root, preparation } = fixture(t);
  await preparation.ensure(["execution"]);
  const env = preparation.env;
  const file = join(root, "crates/sample/input.rs"),
    original = readFileSync(file);
  writeFileSync(file, "// changed without a commit\n");
  assert.throws(
    () => preparedArtifact("execution", root, env),
    /inputs changed/,
  );
  writeFileSync(file, original);
  const context = JSON.parse(env[contextKey]);
  context.tools.cargo = "different toolchain";
  assert.throws(
    () =>
      preparedArtifact("execution", root, {
        ...env,
        [contextKey]: JSON.stringify(context),
      }),
    /toolchain changed/,
  );
  const departed = spawnSync(process.execPath, ["-e", "process.exit(0)"]);
  context.owner = departed.pid;
  context.tools = JSON.parse(env[contextKey]).tools;
  assert.throws(
    () =>
      preparedArtifact("execution", root, {
        ...env,
        [contextKey]: JSON.stringify(context),
      }),
    /owner has exited/,
  );
  preparation.close();
  assert.throws(() => preparedArtifact("execution", root, env), /expired/);
  writeFileSync(
    join(root, ".local-ci-runs/latest.json"),
    JSON.stringify({ status: "passed" }),
  );
  assert.throws(
    () => preparedArtifact("execution", root, {}),
    /prepared inputs required/,
  );
});

test("failed preparation is recorded once and blocks dependent consumers", async (t) => {
  const { preparation, calls } = fixture(t, true);
  await assert.rejects(preparation.ensure(["execution"]), /cargo .*: 1/);
  await assert.rejects(
    preparation.ensure(["execution"]),
    (error) => error.blocked === "execution",
  );
  assert.equal(calls.length, 1);
  let invoked = 0;
  const results = await executeSteps(
    [
      { name: "dependent", command: "node", args: [], selected: true },
      { name: "independent", command: "node", args: [], selected: true },
    ],
    "/unused",
    async (_command, _args, _options) =>
      invoked++ === 0
        ? { status: null, blocked: "preparation:execution" }
        : { status: 0 },
  );
  assert.equal(results[0].outcome, "skipped");
  assert.equal(results[0].status, null);
  assert.equal(results[1].outcome, "passed");
});

test("untracked source affects provenance; caches and generated output do not", (t) => {
  const { root } = fixture(t),
    baseline = sourceDigest(root);
  mkdirSync(join(root, "packages/sample/dist"), { recursive: true });
  // An owner directory itself is an input; compare after it exists.
  const owner = sourceDigest(root);
  writeFileSync(join(root, "packages/sample/dist/output.js"), "generated");
  assert.equal(sourceDigest(root), owner);
  writeFileSync(join(root, "crates/sample/new.rs"), "// untracked");
  assert.notEqual(sourceDigest(root), owner);
  assert.notEqual(owner, baseline);
});

test("capturing Cargo JSON retains cancellation and nonzero outcomes", async () => {
  const env = { ...process.env };
  delete env._AGENT_BUILD_LEASE;
  const success = await runCommand(
    process.execPath,
    ["-e", "console.log(JSON.stringify({reason:'compiler-artifact'}))"],
    { env, stdio: "inherit", capture: true },
  );
  assert.equal(success.status, 0);
  assert.equal(JSON.parse(success.stdout).reason, "compiler-artifact");
  const controller = new AbortController();
  controller.abort("SIGTERM");
  const cancelled = await runCommand(
    process.execPath,
    ["-e", "process.exit(0)"],
    { env, stdio: "inherit", capture: true, signal: controller.signal },
  );
  assert.equal(cancelled.status, null);
  assert.equal(cancelled.signal, "SIGTERM");
});
