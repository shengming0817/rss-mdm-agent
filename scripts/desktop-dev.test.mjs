import assert from "node:assert/strict";
import { once } from "node:events";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  mkdirSync,
  mkdtempSync,
  writeFileSync,
  rmSync,
  readFileSync,
  symlinkSync,
  chmodSync,
  existsSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { test } from "node:test";
import {
  developmentFingerprint,
  ensureDevelopmentRuntime,
} from "./desktop-dev-runtime.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "rss-dev-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const write = (path, value) => {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), value);
  };
  for (const path of [
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates/execution-app/src/lib.rs",
    "scripts/check-execution-bindings.mjs",
    "apps/desktop/src/assistant/execution-types.ts",
    "tests/assistant/execution-fixtures.json",
    "scripts/bundle-ai-host.mjs",
    "scripts/ai-host-artifacts.mjs",
    "scripts/cargo-target.mjs",
    "scripts/desktop-dev-runtime.mjs",
    "scripts/verify-ai-host-runtime.mjs",
  ])
    write(path, "{}");
  for (const path of [
    "apps/ai-host",
    "packages/ai-host",
    "packages/ai-contract",
    "packages/ai-store-sqlite",
    "packages/ai-access",
    "packages/ai-client",
    "packages/ai-adapters/claude",
    "packages/ai-adapters/codex",
    "packages/ai-adapters/deepseek",
  ])
    write(`${path}/src/index.ts`, "original");
  return { root, write };
}
test("fingerprint includes dirty Host, adapter, contract, lock, Node and added/deleted sources, excludes UI and outputs", (t) => {
  const { root, write } = fixture(t);
  const first = developmentFingerprint(root);
  write("apps/desktop/src/App.vue", "UI");
  write("apps/ai-host/dist/cli.js", "output");
  assert.equal(developmentFingerprint(root), first);
  for (const path of [
    "apps/ai-host/src/index.ts",
    "packages/ai-adapters/codex/src/index.ts",
    "packages/ai-contract/schema/runtime.schema.json",
    "pnpm-lock.yaml",
    "package.json",
    "crates/execution-app/src/lib.rs",
    "scripts/check-execution-bindings.mjs",
    "scripts/cargo-target.mjs",
  ]) {
    const before = developmentFingerprint(root);
    write(path, "changed");
    assert.notEqual(developmentFingerprint(root), before, path);
  }
  const before = developmentFingerprint(root);
  write("apps/ai-host/src/new.ts", "new");
  assert.notEqual(developmentFingerprint(root), before);
  rmSync(join(root, "apps/ai-host/src/new.ts"));
  assert.equal(developmentFingerprint(root), before);
});
test("missing and stale runtimes rebuild, unchanged runtime reuses and validates", (t) => {
  const { root, write } = fixture(t);
  let built = 0,
    verified = 0;
  const directory = join(root, "runtime");
  const options = {
    build() {
      built++;
      mkdirSync(directory, { recursive: true });
      write(
        "runtime/manifest.json",
        JSON.stringify({
          kind: "development",
          status: "passed",
          developmentFingerprint: developmentFingerprint(root),
        }),
      );
    },
    verify() {
      verified++;
      return JSON.parse(readFileSync(join(directory, "manifest.json"), "utf8"));
    },
  };
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 1);
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 1);
  write("apps/ai-host/src/index.ts", "dirty");
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 2);
  assert.equal(verified, 3);
});
test("build failures and invalid runtime abort preparation", (t) => {
  const { root } = fixture(t);
  const directory = join(root, "runtime");
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          throw Error("missing dependency");
        },
      }),
    /missing dependency/,
  );
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {},
        verify() {
          throw Error("invalid runtime");
        },
      }),
    /invalid runtime/,
  );
});

test("preparation rejects edits during a build and a corrupt reused runtime", (t) => {
  const { root, write } = fixture(t);
  const directory = join(root, "runtime");
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          write("apps/ai-host/src/index.ts", "concurrent edit");
        },
        verify() {},
      }),
    /source changed/,
  );
  write(
    "runtime/manifest.json",
    JSON.stringify({
      kind: "development",
      developmentFingerprint: developmentFingerprint(root),
    }),
  );
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          assert.fail("must not rebuild unchanged cache");
        },
        verify() {
          throw Error("runtime tree integrity mismatch");
        },
      }),
    /integrity mismatch/,
  );
});

test("invalid override fails before Tauri starts with actionable stage diagnostics", () => {
  const result = spawnSync(
    process.execPath,
    [fileURLToPath(new URL("./desktop-dev.mjs", import.meta.url))],
    {
      encoding: "utf8",
      env: { ...process.env, RSS_AI_HOST_RUNTIME: "" },
    },
  );
  assert.equal(result.status, 1);
  assert.match(result.stderr, /AI Host override validation failed/);
  assert.match(result.stderr, /non-empty runtime directory/);
});

test("release stage rejects development, absent and unknown manifest kinds", (t) => {
  const { root, write } = fixture(t);
  for (const name of [
    "stage-desktop-runtime.mjs",
    "ai-host-artifacts.mjs",
    "cargo-target.mjs",
  ]) {
    write(`scripts/${name}`, readFileSync(new URL(name, import.meta.url)));
  }
  symlinkSync(
    fileURLToPath(new URL("../node_modules", import.meta.url)),
    join(root, "node_modules"),
  );
  for (const kind of ["development", undefined, "unknown"]) {
    write(
      ".local-ci-runs/ai-host-runtime/manifest.json",
      JSON.stringify({ status: "passed", kind }),
    );
    const result = spawnSync(
      process.execPath,
      [join(root, "scripts/stage-desktop-runtime.mjs")],
      {
        cwd: root,
        encoding: "utf8",
        env: { ...process.env, CI_BASE: "develop" },
      },
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /successfully built runtime/);
    assert.doesNotMatch(result.stderr, /TypeError/);
  }
});

// ref: Node.js lib/child_process.js@v24.14.1 (IPC ready and close/reaping).
function processFixture(t, mode = "ready") {
  const root = mkdtempSync(join(tmpdir(), "rss-dev-process-"));
  mkdirSync(join(root, "bin"));
  mkdirSync(join(root, "apps/desktop"), { recursive: true });
  writeFileSync(join(root, "package.json"), JSON.stringify({ type: "module" }));
  writeFileSync(
    join(root, "grandchild.mjs"),
    `
    import {writeSync} from 'node:fs';
    for (const signal of ['SIGTERM','SIGHUP']) process.on(signal,()=>{
      writeSync(1,JSON.stringify({kind:'signal',role:'grandchild',pid:process.pid,signal})+'\\n');
      process.exit(0);
    });
    setInterval(()=>{},1000);
    if (${JSON.stringify(mode)} !== 'not-ready') process.send({kind:'ready',pid:process.pid});
  `,
  );
  writeFileSync(
    join(root, "bin/pnpm"),
    `#!${process.execPath}
    import {spawn} from 'node:child_process';
    import {writeSync} from 'node:fs';
    const emit=(v)=>writeSync(1,JSON.stringify(v)+'\\n');
    emit({kind:'parent',pid:process.pid});
    if (${JSON.stringify(mode)} === 'early-exit') {
      writeSync(1,'startup stdout sentinel\\n');
      writeSync(2,'startup stderr sentinel\\n');
      process.exit(17);
    }
    const child=spawn(process.execPath,[${JSON.stringify(join(root, mode === "child-failure" ? "missing.mjs" : "grandchild.mjs"))}],{stdio:['ignore','inherit','inherit','ipc']});
    emit({kind:'child',pid:child.pid});
    child.on('error',(error)=>{writeSync(2,String(error));process.exitCode=19;});
    let stopping;
    for (const signal of ['SIGTERM','SIGHUP']) process.on(signal,()=>{
      emit({kind:'signal',role:'parent',pid:process.pid,signal});
      stopping ??= signal;
    });
    child.on('message',(value)=>{if(value.kind==='ready') emit({kind:'ready',parent:process.pid,child:child.pid});});
    child.on('close',(code)=>{
      if (stopping) {
        process.removeAllListeners(stopping);
        process.kill(process.pid,stopping);
      } else process.exitCode=code ?? 19;
    });
  `,
  );
  chmodSync(join(root, "bin/pnpm"), 0o755);
  const module = fileURLToPath(
    new URL("desktop-dev-process.mjs", import.meta.url),
  );
  const wrapper = spawn(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import {runDesktop} from ${JSON.stringify(module)}; process.exitCode=await runDesktop(${JSON.stringify(root)},'/runtime');`,
    ],
    {
      env: { ...process.env, PATH: `${join(root, "bin")}:${process.env.PATH}` },
      stdio: "pipe",
    },
  );
  const events = [],
    state = { stdout: "", stderr: "", closed: false };
  let pending = "";
  wrapper.stdout.on("data", (data) => {
    state.stdout += data;
    pending += data;
    let end;
    while ((end = pending.indexOf("\n")) >= 0) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      try {
        events.push(JSON.parse(line));
      } catch {
        /* fixture diagnostics */
      }
    }
  });
  wrapper.stderr.on("data", (data) => {
    state.stderr += data;
  });
  wrapper.on("error", (error) => {
    state.error = String(error);
  });
  wrapper.on("exit", (code, signal) => {
    Object.assign(state, { code, signal });
  });
  wrapper.on("close", () => {
    state.closed = true;
  });
  const diagnostic = (phase) =>
    JSON.stringify({ phase, wrapper: wrapper.pid, ...state, events });
  const pids = () =>
    events
      .filter((v) => ["parent", "child"].includes(v.kind))
      .map((v) => v.pid)
      .filter(Number.isInteger);
  const gone = (pid) => {
    try {
      process.kill(pid, 0);
      return false;
    } catch (error) {
      if (error.code === "ESRCH") return true;
      throw error;
    }
  };
  async function waitFor(predicate, phase, budget = 5000, permitClose = false) {
    const deadline = performance.now() + budget;
    while (!predicate()) {
      if ((!permitClose && state.closed) || state.error)
        throw Error(`early exit: ${diagnostic(phase)}`);
      if (performance.now() >= deadline)
        throw Error(`timeout: ${diagnostic(phase)}`);
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
  }
  const parent = () => events.find((v) => v.kind === "parent")?.pid;
  async function absent() {
    await waitFor(
      () =>
        state.closed && pids().every(gone) && (!parent() || gone(-parent())),
      "cleanup",
      7000,
      true,
    );
  }
  t.after(async () => {
    try {
      if (!state.closed) wrapper.kill("SIGTERM");
      try {
        await absent();
      } catch {
        if (parent() && !gone(-parent())) process.kill(-parent(), "SIGKILL");
        if (!state.closed) wrapper.kill("SIGKILL");
        await absent();
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
  return {
    wrapper,
    state,
    events,
    diagnostic,
    waitFor,
    absent,
    ready: (budget = 5000) =>
      waitFor(() => events.some((v) => v.kind === "ready"), "ready", budget),
  };
}

for (const [signal, expectedCode] of [
  ["SIGTERM", 143],
  ["SIGHUP", 129],
])
  test(
    `${signal} to the wrapper cleans pnpm and its grandchild process group`,
    { skip: process.platform === "win32" },
    async (t) => {
      const f = processFixture(t);
      await f.ready();
      f.wrapper.kill(signal);
      await f.waitFor(
        () =>
          ["parent", "grandchild"].every((role) =>
            f.events.some(
              (e) =>
                e.kind === "signal" && e.role === role && e.signal === signal,
            ),
          ),
        "signal forwarding",
        5000,
        true,
      );
      await f.absent();
      assert.equal(f.state.code, expectedCode, f.diagnostic("exit code"));
    },
  );

test(
  "startup early exit reports code and both output streams",
  { skip: process.platform === "win32" },
  async (t) => {
    const f = processFixture(t, "early-exit");
    await assert.rejects(f.ready(), (error) => {
      assert.match(error.message, /early exit/);
      assert.match(error.message, /"code":17/);
      assert.match(error.message, /startup stdout sentinel/);
      assert.match(error.message, /startup stderr sentinel/);
      return true;
    });
    await f.absent();
  },
);
test(
  "grandchild startup failure is diagnosed and reaped",
  { skip: process.platform === "win32" },
  async (t) => {
    const f = processFixture(t, "child-failure");
    await assert.rejects(f.ready(), /early exit:.*MODULE_NOT_FOUND/);
    await f.absent();
  },
);
test(
  "startup timeout still cleans the already-created process tree",
  { skip: process.platform === "win32" },
  async (t) => {
    const f = processFixture(t, "not-ready");
    await f.waitFor(() => f.events.some((v) => v.kind === "child"), "spawn");
    await assert.rejects(f.ready(100), /timeout:.*ready/);
    f.wrapper.kill("SIGTERM");
    await f.absent();
  },
);

test("reject runtime built from B when source changes A to B to A", (t) => {
  const { root, write } = fixture(t);
  const directory = join(root, "runtime");
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          write("apps/ai-host/src/index.ts", "B");
          write(
            "runtime/manifest.json",
            JSON.stringify({
              kind: "development",
              status: "passed",
              developmentFingerprint: developmentFingerprint(root),
            }),
          );
          write("apps/ai-host/src/index.ts", "original");
        },
        verify() {
          return JSON.parse(
            readFileSync(join(directory, "manifest.json"), "utf8"),
          );
        },
      }),
    /provenance mismatch/,
  );
});

test("preparation admits only verified passed development provenance", (t) => {
  const { root, write } = fixture(t),
    directory = join(root, "runtime");
  const valid = {
    kind: "development",
    status: "passed",
    developmentFingerprint: developmentFingerprint(root),
  };
  write("runtime/manifest.json", JSON.stringify(valid));
  for (const malformed of [
    { ...valid, kind: "release" },
    { ...valid, status: "failed" },
    { ...valid, developmentFingerprint: undefined },
  ])
    assert.throws(
      () =>
        ensureDevelopmentRuntime(root, directory, {
          build() {
            assert.fail("cache matches");
          },
          verify() {
            return malformed;
          },
        }),
      /provenance mismatch/,
    );
});

test(
  "cancelling the preparation process group releases its lock without launching Tauri",
  { skip: process.platform === "win32", timeout: 10000 },
  async (t) => {
    const { root, write } = fixture(t);
    write(
      "scripts/desktop-dev.mjs",
      readFileSync(new URL("desktop-dev.mjs", import.meta.url), "utf8"),
    );
    write(
      "scripts/desktop-dev-runtime.mjs",
      `
    import { spawnSync } from 'node:child_process';
    import { writeFileSync } from 'node:fs';
    export function ensureDevelopmentRuntime(root) {
      writeFileSync(root + '/preparing', 'ready');
      const result = spawnSync(process.execPath, ['-e', 'setInterval(()=>{},1000)']);
      if (result.status !== 0) throw Error('preparation stopped');
      return '/runtime';
    }
    export const verifyDevelopmentRuntime = ensureDevelopmentRuntime;
  `,
    );
    write(
      "scripts/desktop-dev-process.mjs",
      `
    import { writeFileSync } from 'node:fs';
    export async function runDesktop(root) { writeFileSync(root + '/launched', 'unexpected'); return 0; }
  `,
    );
    const child = spawn(
      process.execPath,
      [join(root, "scripts/desktop-dev.mjs")],
      { detached: true, stdio: "ignore" },
    );
    const exited = once(child, "exit");
    t.after(() => {
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
      }
    });
    const deadline = Date.now() + 5000;
    while (!existsSync(join(root, "preparing"))) {
      assert.equal(child.exitCode, null);
      assert.ok(Date.now() < deadline);
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    process.kill(-child.pid, "SIGTERM");
    const [code] = await exited;
    assert.notEqual(code, 0);
    assert.equal(existsSync(join(root, ".cache/desktop-dev.lock")), false);
    assert.equal(existsSync(join(root, "launched")), false);
  },
);
