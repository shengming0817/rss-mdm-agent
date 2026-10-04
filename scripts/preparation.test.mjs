import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync, spawnSync, spawn } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
  readdirSync,
  chmodSync,
  symlinkSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  Preparation,
  contextKey,
  preparedArtifact,
  sourceDigest,
  assertPreparedRun,
  treeDigest,
  withPreparation,
} from "./preparation.mjs";
import { executeSteps, runCommand } from "./ci-plan.mjs";

function fixture(t, failed = false, setup = () => {}) {
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
  setup(root);
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
      if (command === "pnpm" && args.includes("list")) {
        const selected = args[args.indexOf("--filter") + 1].replace(
          /\.\.\.$/,
          "",
        );
        const names =
          selected === "@rss-mdm-agent/first"
            ? ["producer", "first"]
            : ["producer", "second"];
        return {
          status: 0,
          stdout: JSON.stringify(
            names.map((name) => ({
              name: `@rss-mdm-agent/${name}`,
              path: join(root, "packages", name),
            })),
          ),
        };
      }
      if (command === "pnpm") {
        for (let index = 0; index < args.length; index++)
          if (args[index] === "--filter") {
            const name = args[index + 1].split("/").at(-1);
            mkdirSync(join(root, "packages", name, "dist"), {
              recursive: true,
            });
            writeFileSync(
              join(root, "packages", name, "dist/index.js"),
              `export const owner = ${JSON.stringify(name)};`,
            );
          }
        return { status: 0 };
      }
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

test("shared Node dependency closure compiles each package once and rejects changed dist", async (t) => {
  const { root, preparation, calls } = fixture(t, false, (root) => {
    mkdirSync(join(root, "apps"));
    for (const [name, dependencies] of [
      ["producer", {}],
      ["first", { "@rss-mdm-agent/producer": "workspace:*" }],
      ["second", { "@rss-mdm-agent/producer": "workspace:*" }],
    ]) {
      mkdirSync(join(root, "packages", name), { recursive: true });
      writeFileSync(
        join(root, "packages", name, "package.json"),
        JSON.stringify({
          name: `@rss-mdm-agent/${name}`,
          dependencies,
          scripts: { build: "compiler" },
        }),
      );
    }
  });
  await preparation.ensure([
    "node:@rss-mdm-agent/first",
    "node:@rss-mdm-agent/second",
    "node:@rss-mdm-agent/first",
  ]);
  const compiled = calls
    .filter((call) => call.includes("build"))
    .flatMap(([, ...args]) =>
      args.filter((_, index) => args[index - 1] === "--filter"),
    );
  assert.deepEqual(
    compiled.sort(),
    ["first", "producer", "second"].map((name) => `@rss-mdm-agent/${name}`),
  );
  assert.deepEqual(Object.keys(preparation.tools).sort(), ["node", "pnpm"]);
  const path = preparedArtifact(
    "node:@rss-mdm-agent/producer",
    root,
    preparation.env,
  );
  writeFileSync(join(path, "orphan.js"), "stale compiler output");
  await assert.rejects(
    preparation.ensure(["node:@rss-mdm-agent/first"]),
    /missing or changed/,
  );
  assert.throws(
    () =>
      preparedArtifact("node:@rss-mdm-agent/producer", root, preparation.env),
    /missing or changed/,
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
  await assert.rejects(preparation.ensure(["execution"]), /missing or changed/);
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
  const owner = sourceDigest(root);
  assert.equal(owner, baseline);
  writeFileSync(join(root, "packages/sample/dist/output.js"), "generated");
  assert.equal(sourceDigest(root), owner);
  writeFileSync(join(root, "crates/sample/new.rs"), "// untracked");
  assert.notEqual(sourceDigest(root), owner);
  assert.equal(owner, baseline);
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

test("first Tauri generated permissions leave provenance stable; handwritten permissions remain inputs", (t) => {
  const { root } = fixture(t);
  const baseline = sourceDigest(root);
  const directory = join(root, "apps/desktop/src-tauri/permissions");
  mkdirSync(join(directory, "autogenerated"), { recursive: true });
  writeFileSync(join(directory, "autogenerated/output.toml"), "generated");
  assert.equal(sourceDigest(root), baseline);
  const permission = join(directory, "manual.toml");
  writeFileSync(permission, "manual");
  const added = sourceDigest(root);
  assert.notEqual(added, baseline);
  writeFileSync(permission, "changed");
  assert.notEqual(sourceDigest(root), added);
  rmSync(permission);
  assert.equal(sourceDigest(root), baseline);
});

test("deliverable publication guard rejects source/toolchain changes and cancellation", async (t) => {
  const { root, preparation } = fixture(t);
  await preparation.ensure(["execution"]);
  const env = preparation.env;
  const controller = new AbortController();
  assert.doesNotThrow(() => assertPreparedRun(controller.signal, root, env));
  writeFileSync(
    join(root, "crates/sample/input.rs"),
    "changed after validation",
  );
  assert.throws(
    () => assertPreparedRun(controller.signal, root, env),
    /inputs changed/,
  );
  writeFileSync(join(root, "crates/sample/input.rs"), "// baseline\n");
  const value = JSON.parse(env[contextKey]);
  value.tools.cargo = "replaced after validation";
  assert.throws(
    () =>
      assertPreparedRun(controller.signal, root, {
        ...env,
        [contextKey]: JSON.stringify(value),
      }),
    /toolchain changed/,
  );
  controller.abort("SIGTERM");
  assert.throws(
    () => assertPreparedRun(controller.signal, root, env),
    /cancelled/,
  );
});

test("artifact entry framing rejects payloads that impersonate deleted files or links", async (t) => {
  const { root, preparation } = fixture(t);
  await preparation.ensure(["execution"]);
  const directory = join(preparation.directory, "worker");
  mkdirSync(directory);
  writeFileSync(join(directory, "a.js"), "first");
  writeFileSync(join(directory, "b.js"), "second");
  preparation.record("worker", directory);
  const digest = treeDigest(directory);
  writeFileSync(join(directory, "a.js"), "first./b.js\0second");
  rmSync(join(directory, "b.js"));
  assert.notEqual(treeDigest(directory), digest);
  assert.throws(
    () => preparedArtifact("worker", root, preparation.env),
    /changed prepared artifact/,
  );
  writeFileSync(join(directory, "a.js"), "link\0destination");
  const fileDigest = treeDigest(directory);
  rmSync(join(directory, "a.js"));
  symlinkSync("destination", join(directory, "a.js"));
  assert.notEqual(treeDigest(directory), fileDigest);
});

test(
  "a hanging tool probe is forcibly bounded and releases its run after cancellation",
  { timeout: 15000 },
  async (t) => {
    const { root } = fixture(t);
    const bin = join(root, "bin");
    mkdirSync(bin);
    const marker = join(root, "probe-started");
    writeFileSync(
      join(bin, "cargo"),
      `#!${process.execPath}\nimport {writeFileSync} from 'node:fs';writeFileSync(${JSON.stringify(marker)},String(process.pid));process.on('SIGTERM',()=>{});setInterval(()=>{},1000);`,
    );
    chmodSync(join(bin, "cargo"), 0o755);
    const module = new URL("./preparation.mjs", import.meta.url).href;
    const env = { ...process.env, PATH: bin + ":" + process.env.PATH };
    delete env[contextKey];
    delete env._AGENT_BUILD_LEASE;
    const child = spawn(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        `import {withPreparation} from ${JSON.stringify(module)};await withPreparation(['execution'],()=>{throw Error('must not consume')},{directory:process.argv[1]});`,
        root,
      ],
      { env, stdio: ["ignore", "pipe", "pipe"] },
    );
    let stderr = "";
    child.stderr.on("data", (bytes) => (stderr += bytes));
    const closed = new Promise((resolve, reject) => {
      child.on("error", reject);
      child.on("close", (code, signal) => resolve({ code, signal }));
    });
    t.after(() => {
      child.kill("SIGKILL");
    });
    const start = performance.now();
    while (!readdirSync(root).includes("probe-started")) {
      assert.ok(performance.now() - start < 3000, "probe did not start");
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    child.kill("SIGTERM");
    const result = await closed;
    assert.notEqual(result.code, 0);
    assert.match(stderr, /ETIMEDOUT/);
    assert.ok(
      performance.now() - start < 13000,
      "probe exceeded its bounded timeout",
    );
    const probePid = Number(readFileSync(marker, "utf8"));
    assert.throws(() => process.kill(probePid, 0), { code: "ESRCH" });
    assert.deepEqual(
      readdirSync(join(root, ".local-ci-runs")),
      [],
      "owning run cleans preparation directory",
    );
  },
);

test("the owning runner keeps artifacts alive across awaits and disposes them after the consumer", async (t) => {
  const { root, preparation, calls } = fixture(t);
  const previous = process.env[contextKey];
  let path;
  const code = await withPreparation(
    ["execution"],
    async (env) => {
      await new Promise((resolve) => setTimeout(resolve, 20));
      path = preparedArtifact("execution", root, env);
      assert.ok(readFileSync(path).length);
      await new Promise((resolve) => setTimeout(resolve, 20));
      assert.equal(preparedArtifact("execution", root, env), path);
      return 7;
    },
    { directory: root, env: preparation.baseEnv, execute: preparation.execute },
  );
  assert.equal(code, 7);
  assert.equal(calls.length, 1);
  assert.equal(process.env[contextKey], previous);
  assert.throws(() => readFileSync(path), { code: "ENOENT" });
});
