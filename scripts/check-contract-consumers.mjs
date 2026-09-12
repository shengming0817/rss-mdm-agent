import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { sameCommittedSource, sourceState } from "./source-state.mjs";

const names = ["execution-contract", "ai-session-contract"];
function cargo(args, cwd, env, execute, receipt, capture = false) {
  const command = ["cargo", ...args];
  receipt.command = command;
  const result = execute("cargo", args, {
    cwd,
    env,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.status !== 0 || result.error || result.signal) {
    receipt.failure = {
      command,
      status: result.status ?? null,
      signal: result.signal ?? null,
      errorCode: result.error?.code ?? null,
    };
    throw new Error("cargo step failed");
  }
  return result.stdout;
}
function checkOne(root, name, serdeVersion, execute, receipt) {
  let dir;
  try {
    dir = realpathSync(mkdtempSync(join(tmpdir(), `${name}-consumer-`)));
    const crate = join(root, "crates", name);
    const env = {
      ...process.env,
      CARGO_TARGET_DIR: join(dir, "target"),
      RUSTFLAGS: "",
      CARGO_ENCODED_RUSTFLAGS: "",
      RUSTC_WRAPPER: "",
      RUSTC_WORKSPACE_WRAPPER: "",
    };
    mkdirSync(join(dir, "src"));
    writeFileSync(
      join(dir, "Cargo.toml"),
      `[package]\nname = "isolated-consumer"\nversion = "0.0.0"\nedition = "2021"\n[workspace]\n[dependencies]\n${name} = { path = ${JSON.stringify(crate)}, default-features = false }\nserde_json = ${JSON.stringify(serdeVersion)}\n`,
    );
    copyFileSync(
      join(
        crate,
        `examples/${name === "execution-contract" ? "execution" : "ai-session"}-consumer.rs`,
      ),
      join(dir, "src/main.rs"),
    );
    const fixture = name === "execution-contract" ? "plan.json" : "events.json";
    copyFileSync(
      join(crate, "tests/fixtures", fixture),
      join(dir, "input.json"),
    );
    const args = [join(dir, "input.json")];
    if (name === "execution-contract") {
      copyFileSync(
        join(crate, "tests/fixtures/plan.sha256"),
        join(dir, "digest.txt"),
      );
      args.push(join(dir, "digest.txt"));
    }
    cargo(["generate-lockfile", "--offline"], dir, env, execute, receipt);
    receipt.lockSha256 = createHash("sha256")
      .update(readFileSync(join(dir, "Cargo.lock")))
      .digest("hex");
    const metadata = JSON.parse(
      cargo(
        ["metadata", "--offline", "--locked", "--format-version", "1"],
        dir,
        env,
        execute,
        receipt,
        true,
      ),
    );
    receipt.dependencies = metadata.packages.map((p) => ({
      name: p.name,
      version: p.version,
      source: p.source,
    }));
    receipt.stage = "isolation";
    if (
      resolve(metadata.workspace_root) !== dir ||
      resolve(metadata.target_directory) !== env.CARGO_TARGET_DIR
    )
      throw new Error("workspace/target isolation failed");
    for (const pkg of metadata.packages) {
      if (
        pkg.source === null &&
        ![join(dir, "Cargo.toml"), join(crate, "Cargo.toml")].includes(
          pkg.manifest_path,
        )
      )
        throw new Error("unexpected source dependency");
      if (pkg.source !== null && !pkg.source.startsWith("registry+"))
        throw new Error("non-registry dependency");
      if (
        /tauri|sqlx|sqlite|prmonitor|^ai-(codex|claude|cursor)$/.test(pkg.name)
      )
        throw new Error("unexpected runtime dependency");
    }
    receipt.stage = "run";
    cargo(
      ["run", "--offline", "--locked", "--", ...args],
      dir,
      env,
      execute,
      receipt,
    );
    receipt.passed = true;
  } catch {
    receipt.passed = false;
    receipt.failure ??= { stage: receipt.stage, code: "consumer-check-failed" };
  } finally {
    try {
      if (dir) rmSync(dir, { recursive: true, force: true });
      receipt.cleanup = dir ? "removed" : "not-created";
    } catch {
      receipt.cleanup = "failed";
      receipt.passed = false;
      receipt.failure ??= { stage: "cleanup", code: "consumer-cleanup-failed" };
    }
  }
}
// The process seam is injectable for deterministic failure tests, not a product API.
export function checkContractConsumers(root, execute = spawnSync) {
  root = realpathSync(root);
  const reportDir = join(root, ".local-ci-runs");
  const reportPath = join(reportDir, "contracts.json");
  mkdirSync(reportDir, { recursive: true });
  // Invalidate prior success even if writing this run's report later fails.
  rmSync(reportPath, { force: true });
  const report = {
    status: "running",
    startedAt: new Date().toISOString(),
    source: null,
    consumers: [],
  };
  const temporary = join(reportDir, `contracts-${process.pid}.tmp`);
  function publish() {
    try {
      writeFileSync(temporary, JSON.stringify(report, null, 2));
      renameSync(temporary, reportPath);
    } finally {
      rmSync(temporary, { force: true });
    }
  }
  publish();
  try {
    report.source = sourceState(root);
    const owner = JSON.parse(
      cargo(
        ["metadata", "--no-deps", "--locked", "--format-version", "1"],
        root,
        process.env,
        execute,
        report,
        true,
      ),
    );
    for (const name of names) {
      const receipt = { name, stage: "prepare", passed: false };
      report.consumers.push(receipt);
      const version = owner.packages
        .find((p) => p.name === name)
        ?.dependencies.find((d) => d.name === "serde_json")?.req;
      if (!version)
        receipt.failure = {
          stage: "prepare",
          code: "missing-contract-dependency",
        };
      else checkOne(root, name, version, execute, receipt);
      publish();
    }
    report.sourceEnd = sourceState(root);
    const committed = sameCommittedSource(report.source, report.sourceEnd);
    if (!committed)
      report.failure ??= {
        stage: "provenance",
        code: "uncommitted-or-changed-source",
      };
    report.status =
      report.consumers.every((r) => r.passed) && committed
        ? "passed"
        : "failed";
  } catch {
    report.status = "failed";
    report.failure ??= { code: "consumer-preparation-failed" };
  } finally {
    report.finishedAt = new Date().toISOString();
    publish();
  }
  return report;
}
if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  const report = checkContractConsumers(
    fileURLToPath(new URL("../", import.meta.url)),
  );
  console.log(`Isolated contract consumers: ${report.status}`);
  process.exitCode = report.status === "passed" ? 0 : 1;
}
