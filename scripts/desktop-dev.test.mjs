import assert from "node:assert/strict";
import { spawn, spawnSync, execFileSync } from "node:child_process";
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
import { once } from "node:events";
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
  for (const name of ["stage-desktop-runtime.mjs", "ai-host-artifacts.mjs"]) {
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

for (const [signal, expectedCode] of [
  ["SIGTERM", 143],
  ["SIGHUP", 129],
])
  test(`${signal} to the wrapper cleans pnpm and its grandchild process group`, async (t) => {
    const { root, write } = fixture(t);
    mkdirSync(join(root, "apps/desktop"), { recursive: true });
    write(
      "bin/pnpm",
      `#!${process.execPath}\nimport {spawn} from 'node:child_process';\nimport {writeFileSync} from 'node:fs';\nconst child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});\nwriteFileSync(${JSON.stringify(join(root, "pids.json"))},JSON.stringify([process.pid,child.pid]));\nsetInterval(()=>{},1000);\n`,
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
        env: {
          ...process.env,
          PATH: `${join(root, "bin")}:${process.env.PATH}`,
        },
        stdio: "pipe",
      },
    );
    t.after(() => wrapper.kill("SIGKILL"));
    const exit = once(wrapper, "exit");
    const deadline = Date.now() + 5000;
    while (!existsSync(join(root, "pids.json"))) {
      assert.ok(Date.now() < deadline, "child startup");
      await new Promise((r) => setTimeout(r, 20));
    }
    const pids = JSON.parse(readFileSync(join(root, "pids.json")));
    t.after(() => {
      for (const pid of pids) {
        try {
          process.kill(pid, "SIGKILL");
        } catch {}
      }
    });
    wrapper.kill(signal);
    const [code] = await exit;
    assert.equal(code, expectedCode);
    for (const pid of pids)
      assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
  });

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
