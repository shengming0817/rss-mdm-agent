import { execFileSync, spawn, spawnSync } from "node:child_process";
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const runner = new URL("./build-run.py", import.meta.url).pathname;
const onlyMac = { skip: process.platform !== "darwin" };

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "agent-build-run-"));
  const pool = join(root, "pool");
  const home = join(root, "home");
  mkdirSync(home);
  const worktree = (name) => {
    const path = join(root, name);
    mkdirSync(path);
    execFileSync("/usr/bin/git", ["init", "-q", path]);
    return path;
  };
  const env = {
    ...process.env,
    HOME: home,
    AGENT_TARGET_POOL_ROOT: pool,
    AGENT_TARGET_POOL_N: "1",
  };
  for (const key of [
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "_AGENT_BUILD_LEASE",
  ])
    delete env[key];
  const run = (cwd, code, extra = {}) =>
    spawnSync("python3", [runner, "--", process.execPath, "-e", code], {
      cwd,
      env: { ...env, ...extra },
      encoding: "utf8",
      timeout: 10000,
    });
  return {
    root,
    pool,
    env,
    worktree,
    run,
    close: () => rmSync(root, { recursive: true, force: true }),
  };
}

async function holder(f, cwd) {
  const ready = join(f.root, `ready-${Date.now()}-${Math.random()}`);
  const child = spawn(
    "python3",
    [
      runner,
      "--",
      process.execPath,
      "-e",
      `require('fs').writeFileSync(${JSON.stringify(ready)}, process.env.CARGO_TARGET_DIR); setTimeout(() => {}, 30000)`,
    ],
    { cwd, env: f.env, stdio: ["ignore", "ignore", "pipe"] },
  );
  const deadline = Date.now() + 5000;
  while (!existsSync(ready) && child.exitCode === null && Date.now() < deadline)
    await new Promise((resolve) => setTimeout(resolve, 20));
  assert.ok(existsSync(ready), "holder did not acquire its slot");
  return {
    child,
    target: readFileSync(ready, "utf8"),
    done: new Promise((resolve) => child.once("close", resolve)),
  };
}

test(
  "sticky worktree reuses a slot; reassignment wipes old Rust artifacts",
  onlyMac,
  () => {
    const f = fixture();
    try {
      const first = f.worktree("first");
      const second = f.worktree("second");
      const code =
        "const fs=require('fs'); const p=process.env.CARGO_TARGET_DIR; fs.writeFileSync(p+'/artifact','old'); console.log(p)";
      const initial = f.run(first, code);
      assert.equal(initial.status, 0, initial.stderr);
      const target = initial.stdout.trim();
      const again = f.run(first, "console.log(process.env.CARGO_TARGET_DIR)");
      assert.equal(again.status, 0, again.stderr);
      assert.equal(again.stdout.trim(), target);
      assert.ok(existsSync(join(target, "artifact")));
      const reassigned = f.run(
        second,
        "console.log(process.env.CARGO_TARGET_DIR)",
      );
      assert.equal(reassigned.status, 0, reassigned.stderr);
      assert.equal(reassigned.stdout.trim(), target);
      assert.equal(existsSync(join(target, "artifact")), false);
    } finally {
      f.close();
    }
  },
);

test(
  "Node descendants inherit the Rust target and same worktree cannot overlap",
  onlyMac,
  async () => {
    const f = fixture();
    let held;
    try {
      const first = f.worktree("first");
      const second = f.worktree("second");
      held = await holder(f, first);
      const same = f.run(first, "process.exit(0)");
      assert.equal(same.status, 2);
      assert.match(same.stderr, /worktree busy/);
      const full = f.run(second, "process.exit(0)");
      assert.equal(full.status, 2);
      assert.match(full.stderr, /pool full/);
      held.child.kill("SIGTERM");
      assert.equal(await held.done, 143);
      held = null;
      assert.equal(f.run(first, "process.exit(0)").status, 0);
    } finally {
      if (held) {
        held.child.kill("SIGTERM");
        await held.done;
      }
      f.close();
    }
  },
);

test(
  "different worktrees run concurrently until the finite pool is full",
  onlyMac,
  async () => {
    const f = fixture();
    f.env.AGENT_TARGET_POOL_N = "2";
    const held = [];
    try {
      const first = f.worktree("first");
      const second = f.worktree("second");
      const third = f.worktree("third");
      held.push(await holder(f, first), await holder(f, second));
      assert.notEqual(held[0].target, held[1].target);
      const full = f.run(third, "process.exit(0)");
      assert.equal(full.status, 2);
      assert.match(full.stderr, /pool full/);
    } finally {
      for (const item of held) item.child.kill("SIGTERM");
      await Promise.all(held.map((item) => item.done));
      f.close();
    }
  },
);

test(
  "unowned pool and symlink slot are rejected without deleting outside data",
  onlyMac,
  () => {
    const f = fixture();
    try {
      const work = f.worktree("work");
      mkdirSync(f.pool);
      writeFileSync(join(f.pool, "important"), "retain");
      const unowned = f.run(work, "process.exit(0)");
      assert.equal(unowned.status, 2);
      assert.match(unowned.stderr, /unmarked nonempty/);
      assert.ok(existsSync(join(f.pool, "important")));
      rmSync(f.pool, { recursive: true });
      assert.equal(f.run(work, "process.exit(0)").status, 0);
      const outside = join(f.root, "outside");
      mkdirSync(outside);
      writeFileSync(join(outside, "important"), "retain");
      rmSync(join(f.pool, "slot-0"), { recursive: true });
      symlinkSync(outside, join(f.pool, "slot-0"), "dir");
      const linked = f.run(work, "process.exit(0)");
      assert.equal(linked.status, 2);
      assert.match(linked.stderr, /symlink slot/);
      assert.ok(existsSync(join(outside, "important")));
    } finally {
      f.close();
    }
  },
);

test(
  "managed CI rejects external target and wrapper overrides",
  onlyMac,
  () => {
    const f = fixture();
    try {
      const work = f.worktree("work");
      for (const extra of [
        { CARGO_TARGET_DIR: join(f.root, "explicit") },
        { CARGO_BUILD_TARGET_DIR: join(f.root, "explicit") },
        { RUSTC_WRAPPER: "/tmp/wrapper" },
        { AGENT_TARGET_POOL_N: "off" },
      ]) {
        const result = f.run(work, "process.exit(0)", extra);
        assert.equal(result.status, 2, result.stderr);
      }
      assert.equal(existsSync(f.pool), false);
    } finally {
      f.close();
    }
  },
);
