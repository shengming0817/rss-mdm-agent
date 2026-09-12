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
import { checkContractConsumers } from "./check-contract-consumers.mjs";
import { git } from "./source-state.mjs";

const names = ["execution-contract", "ai-session-contract"];
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
  for (const name of names) {
    const path = join(root, "crates", name);
    mkdirSync(join(path, "examples"), { recursive: true });
    mkdirSync(join(path, "tests/fixtures"), { recursive: true });
    writeFileSync(
      join(
        path,
        "examples",
        `${name === "execution-contract" ? "execution" : "ai-session"}-consumer.rs`,
      ),
      "fn main() {}\n",
    );
    for (const file of ["plan.json", "plan.sha256", "events.json"])
      writeFileSync(join(path, "tests/fixtures", file), "fixture");
  }
  mkdirSync(join(root, ".local-ci-runs"));
  writeFileSync(
    join(root, ".local-ci-runs/contracts.json"),
    JSON.stringify({ status: "passed", source: { head: "stale-success" } }),
  );
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
            dependencies: [{ name: "serde_json", req: "=1.0.151" }],
          })),
        }),
      };
    dirs.add(cwd);
    const manifest = readFileSync(join(cwd, "Cargo.toml"), "utf8");
    const name = names.find((name) => manifest.includes(`${name} =`));
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
            {
              name,
              version: "0.1.0",
              source: null,
              manifest_path: join(root, "crates", name, "Cargo.toml"),
            },
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
    const result = checkContractConsumers(root, fake.execute);
    assert.equal(result.status, "failed");
    assert.deepEqual(fake.attempted, names);
    const saved = JSON.parse(
      readFileSync(join(root, ".local-ci-runs/contracts.json"), "utf8"),
    );
    assert.equal(saved.status, "failed");
    assert.notEqual(saved.source.head, "stale-success");
    assert.equal(saved.consumers.length, 2);
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
    const result = checkContractConsumers(root, fake.execute);
    assert.deepEqual(fake.attempted, names);
    assert.equal(result.status, "failed");
    assert.equal(result.consumers[0].failure.signal, "SIGTERM");
    assert.equal(result.consumers[1].failure.errorCode, "ENOENT");
    assert.ok(!JSON.stringify(result).includes("untrusted diagnostic"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
