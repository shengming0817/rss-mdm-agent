import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  existsSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { checkRustConsumers, rustConsumers } from "./check-rust-consumers.mjs";
import { git } from "./source-state.mjs";

const names = [
  "execution-contract",
  "ai-session-contract",
  "execution-interaction",
  "execution-capability",
  "execution-admission",
];
function fixture() {
  const root = realpathSync(
    mkdtempSync(join(tmpdir(), "contract-receipt-test-")),
  );
  execFileSync(git, ["init", "-q", root]);
  execFileSync(
    git,
    [
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "--allow-empty",
      "-qm",
      "fixture",
    ],
    { cwd: root },
  );
  execFileSync(git, ["update-ref", "refs/remotes/origin/develop", "HEAD"], {
    cwd: root,
  });
  for (const spec of rustConsumers) {
    const { name } = spec;
    const path = join(root, "crates", name);
    mkdirSync(join(path, "examples"), { recursive: true });
    mkdirSync(join(path, "tests/fixtures"), { recursive: true });
    writeFileSync(join(path, "examples", spec.example), "fn main() {}\n");
    for (const file of ["plan.json", "plan.sha256", "events.json"])
      writeFileSync(join(path, "tests/fixtures", file), "fixture");
  }
  mkdirSync(join(root, ".local-ci-runs"));
  writeFileSync(
    join(root, ".local-ci-runs/rust-consumers.json"),
    JSON.stringify({ status: "passed", source: { head: "stale-success" } }),
  );
  writeFileSync(join(root, ".gitignore"), ".local-ci-runs/\n");
  execFileSync(git, ["add", "crates", ".gitignore"], { cwd: root });
  execFileSync(
    git,
    [
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "-qm",
      "source fixture",
    ],
    { cwd: root },
  );
  execFileSync(git, ["update-ref", "refs/remotes/origin/develop", "HEAD"], {
    cwd: root,
  });
  return root;
}
function fakeCargo(root, failure) {
  const attempted = [],
    dirs = new Set();
  function execute(command, args, options) {
    assert.equal(command, "cargo");
    const { cwd, env } = options;
    if (args.includes("--no-deps"))
      return {
        status: 0,
        stdout: JSON.stringify({
          packages: names.map((name) => ({
            name,
            dependencies: [
              {
                name: "serde_json",
                req: "=1.0.151",
                source: "registry+https://github.com/rust-lang/crates.io-index",
              },
            ],
          })),
        }),
      };
    dirs.add(cwd);
    const manifest = readFileSync(join(cwd, "Cargo.toml"), "utf8");
    const name = names.find((name) =>
      manifest.includes(`name = "isolated-${name}-consumer"`),
    );
    if (args[0] === "generate-lockfile")
      writeFileSync(join(cwd, "Cargo.lock"), `lock-${name}`);
    if (args[0] === "metadata")
      return {
        status: 0,
        stdout: JSON.stringify({
          workspace_root: cwd,
          target_directory: env.CARGO_TARGET_DIR,
          packages: [
            {
              name: "isolated-consumer",
              version: "0.0.0",
              source: null,
              manifest_path: join(cwd, "Cargo.toml"),
            },
            ...rustConsumers
              .find((spec) => spec.name === name)
              .locals.map((local) => ({
                name: local,
                version: "0.1.0",
                source: null,
                manifest_path: join(root, "crates", local, "Cargo.toml"),
              })),
          ],
        }),
      };
    if (args[0] === "run") {
      attempted.push(name);
      return failure(name);
    }
    return { status: 0, stdout: "" };
  }
  return { execute, attempted, dirs };
}
test("consumer failures are aggregated, replace stale PASS, and clean every temporary directory", () => {
  const root = fixture();
  try {
    const fake = fakeCargo(root, () => ({ status: 17, signal: null }));
    const result = checkRustConsumers(root, fake.execute);
    assert.equal(result.status, "failed");
    assert.deepEqual(fake.attempted, names);
    const saved = JSON.parse(
      readFileSync(join(root, ".local-ci-runs/rust-consumers.json"), "utf8"),
    );
    assert.equal(saved.status, "failed");
    assert.notEqual(saved.source.head, "stale-success");
    assert.equal(saved.consumers.length, 5);
    for (const entry of saved.consumers) {
      assert.equal(entry.passed, false);
      assert.equal(entry.failure.status, 17);
      assert.equal(entry.failure.command[1], "run");
      assert.equal(entry.cleanup, "removed");
    }
    for (const dir of fake.dirs) assert.equal(existsSync(dir), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
test("signals and spawn errors are recorded as failures without blocking the other consumer", () => {
  const root = fixture();
  try {
    const fake = fakeCargo(root, (name) =>
      name === names[0]
        ? { status: null, signal: "SIGTERM" }
        : {
            status: null,
            error: { code: "ENOENT", message: "untrusted diagnostic" },
          },
    );
    const result = checkRustConsumers(root, fake.execute);
    assert.deepEqual(fake.attempted, names);
    assert.equal(result.status, "failed");
    assert.equal(result.consumers[0].failure.signal, "SIGTERM");
    assert.equal(result.consumers[1].failure.errorCode, "ENOENT");
    assert.ok(!JSON.stringify(result).includes("untrusted diagnostic"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("only clean unchanged source can produce a deliverable consumer PASS", () => {
  for (const mode of [
    "clean",
    "dirty-at-start",
    "mutation-during-run",
    "base-drift",
    "head-drift",
  ]) {
    const root = fixture();
    try {
      const input = join(
        root,
        "crates/execution-contract/examples/execution-consumer.rs",
      );
      if (mode === "dirty-at-start")
        writeFileSync(input, "changed before checking");
      const fake = fakeCargo(root, (name) => {
        if (name === names[0] && mode === "mutation-during-run")
          writeFileSync(input, "changed while checking");
        if (name === names[0] && mode === "head-drift")
          execFileSync(
            git,
            [
              "-c",
              "user.name=Fixture",
              "-c",
              "user.email=fixture@example.invalid",
              "commit",
              "--allow-empty",
              "-qm",
              "concurrent committed change",
            ],
            { cwd: root },
          );
        if (name === names[0] && mode === "base-drift")
          execFileSync(
            git,
            ["update-ref", "refs/remotes/origin/develop", "HEAD~1"],
            { cwd: root },
          );
        return { status: 0, stdout: "" };
      });
      const result = checkRustConsumers(root, fake.execute);
      assert.deepEqual(fake.attempted, names);
      assert.equal(result.status, mode === "clean" ? "passed" : "failed", mode);
      if (mode !== "clean")
        assert.equal(result.failure.code, "uncommitted-or-changed-source");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});

test("consumer inventory covers the five independent crates", () => {
  assert.deepEqual(
    rustConsumers.map((spec) => spec.name),
    names,
  );
});

test("unexpected local or git dependencies fail isolation without hiding later consumer results", () => {
  for (const source of [null, "git+https://example.invalid/forbidden"]) {
    const root = fixture();
    try {
      const fake = fakeCargo(root, () => ({ status: 0, stdout: "" }));
      const execute = (command, args, options) => {
        const result = fake.execute(command, args, options);
        if (args[0] === "metadata" && !args.includes("--no-deps")) {
          const metadata = JSON.parse(result.stdout);
          metadata.packages.push({
            name: "forbidden-owner",
            version: "0.1.0",
            source,
            manifest_path: join(root, "outside", "Cargo.toml"),
          });
          result.stdout = JSON.stringify(metadata);
        }
        return result;
      };
      const report = checkRustConsumers(root, execute);
      assert.equal(report.status, "failed");
      assert.equal(report.consumers.length, 5);
      assert.ok(
        report.consumers.every(
          (r) => !r.passed && r.failure.stage === "isolation",
        ),
      );
      assert.equal(fake.attempted.length, 0);
      for (const dir of fake.dirs) assert.equal(existsSync(dir), false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});
