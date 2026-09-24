import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  copyFileSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
const git = process.platform === "win32" ? "git" : "/usr/bin/git";

// Mutate real public structs and compile the production authorization libraries. A text-pattern
// assertion or a compile_fail example would not prove that production uses the guard.
test("new execution fields require an explicit admission/capability decision", () => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const dir = mkdtempSync(join(tmpdir(), "execution-evolution-"));
  try {
    const files = execFileSync(
      git,
      [
        "ls-files",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates",
        "apps/desktop/src-tauri",
      ],
      { cwd: root, encoding: "utf8" },
    )
      .trim()
      .split("\n");
    for (const file of files) {
      mkdirSync(dirname(join(dir, file)), { recursive: true });
      copyFileSync(join(root, file), join(dir, file));
    }
    const check = (name) =>
      spawnSync(
        "cargo",
        [
          "check",
          "--offline",
          "--locked",
          "--lib",
          "-p",
          name,
          "--message-format=json",
        ],
        {
          cwd: dir,
          encoding: "utf8",
          maxBuffer: 16 * 1024 * 1024,
          env: {
            ...process.env,
            CARGO_TARGET_DIR: join(root, "target", "execution-evolution"),
            RUSTFLAGS: "",
            CARGO_ENCODED_RUSTFLAGS: "",
            RUSTC_WRAPPER: "",
            RUSTC_WORKSPACE_WRAPPER: "",
          },
        },
      );
    for (const consumer of ["execution-admission", "execution-capability"]) {
      const baseline = check(consumer);
      assert.equal(baseline.status, 0, baseline.stderr);
      for (const type of [
        "PlanSpec",
        "ExecutionRequest",
        "Constraints",
        ...(consumer === "execution-capability"
          ? ["EnvironmentSnapshot", "LaunchSpec"]
          : []),
      ]) {
        const owner =
          type === "EnvironmentSnapshot" ? consumer : "execution-contract";
        const path = join(dir, "crates", owner, "src/model.rs");
        const original = readFileSync(path, "utf8");
        try {
          writeFileSync(
            path,
            original.replace(
              `pub struct ${type} {`,
              `pub struct ${type} {\n    /// Mutation probe: a new security requirement.\n    pub evolution_probe: bool,`,
            ),
          );
          const result = check(consumer);
          assert.notEqual(
            result.status,
            0,
            `${consumer} silently ignores new ${type} field`,
          );
          const messages = result.stdout
            .trim()
            .split("\n")
            .map((line) => JSON.parse(line));
          assert.ok(
            messages.some(
              (m) =>
                m.reason === "compiler-message" &&
                m.message.code?.code === "E0027" &&
                m.message.spans.some((s) =>
                  s.file_name.includes(`crates/${consumer}/src/`),
                ),
            ),
            `${consumer}/${type}: must fail at production exhaustive pattern\n${result.stderr}`,
          );
        } finally {
          writeFileSync(path, original);
        }
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
