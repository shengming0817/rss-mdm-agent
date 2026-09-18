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

// One explicit inventory for real public-API consumers; no runtime plugin discovery.
export const rustConsumers = [
  {
    name: "execution-contract",
    example: "execution-consumer.rs",
    locals: ["execution-contract"],
    registry: ["serde_json"],
    fixtures: [
      "crates/execution-contract/tests/fixtures/plan.json",
      "crates/execution-contract/tests/fixtures/plan.sha256",
    ],
  },
  {
    name: "ai-session-contract",
    example: "ai-session-consumer.rs",
    locals: ["ai-session-contract"],
    registry: ["serde_json"],
    fixtures: ["crates/ai-session-contract/tests/fixtures/events.json"],
  },
  {
    name: "execution-interaction",
    example: "interaction-consumer.rs",
    locals: ["execution-interaction"],
    registry: ["serde_json"],
    fixtures: [],
  },
  {
    name: "execution-capability",
    example: "capability-consumer.rs",
    locals: ["execution-capability", "execution-contract"],
    registry: [],
    fixtures: ["crates/execution-contract/tests/fixtures/plan.json"],
  },
  {
    name: "execution-admission",
    example: "admission-consumer.rs",
    locals: ["execution-admission", "execution-contract"],
    registry: [],
    fixtures: ["crates/execution-contract/tests/fixtures/plan.json"],
  },
  {
    name: "execution-approval",
    example: "approval-consumer.rs",
    locals: ["execution-approval", "execution-admission", "execution-contract"],
    registry: [],
    fixtures: ["crates/execution-contract/tests/fixtures/plan.json"],
  },
  {
    name: "execution-lifecycle",
    example: "lifecycle-consumer.rs",
    locals: ["execution-lifecycle", "execution-contract"],
    registry: ["serde_json"],
    fixtures: ["crates/execution-contract/tests/fixtures/plan.json"],
  },
  {
    name: "service-catalog",
    example: "catalog-consumer.rs",
    locals: ["service-catalog", "execution-contract"],
    registry: ["serde_json"],
    fixtures: [
      "crates/service-catalog/tests/fixtures/catalog.json",
      "crates/service-catalog/tests/fixtures/catalog.sha256",
    ],
  },
  {
    name: "script-plan",
    example: "script-plan-consumer.rs",
    locals: ["script-plan", "execution-contract"],
    registry: ["serde_json"],
    fixtures: [],
  },
  {
    name: "software-plan",
    example: "software-plan-consumer.rs",
    locals: ["software-plan", "execution-contract"],
    registry: [],
    fixtures: [],
  },
  {
    name: "execution-mcp",
    example: "mcp-consumer.rs",
    locals: ["execution-mcp", "execution-contract", "service-catalog"],
    registry: ["serde_json", "sha2", "tokio", "tokio-util"],
    fixtures: [
      "crates/service-catalog/tests/fixtures/catalog.json",
      "crates/execution-contract/tests/fixtures/plan.json",
    ],
  },
];
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
// Closed failure codes retain classification without serializing paths or exception text.
class ConsumerFailure extends Error {
  constructor(code, packageName) {
    super(code);
    this.code = code;
    this.packageName = packageName;
  }
}
function checkOne(root, spec, owner, execute, receipt) {
  const { name } = spec;
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
    const localDependencies = spec.locals.map(
      (local) =>
        `${local} = { path = ${JSON.stringify(join(root, "crates", local))}, default-features = false }`,
    );
    const packageMetadata = owner.packages.find((p) => p.name === name);
    if (!packageMetadata)
      throw new ConsumerFailure("missing-consumer-owner", name);
    const registryDependencies = spec.registry.map((dependency) => {
      const declaration = packageMetadata.dependencies.find(
        (d) => d.name === dependency && d.source?.startsWith("registry+"),
      );
      if (!declaration?.req)
        throw new ConsumerFailure("missing-registry-dependency", dependency);
      return `${dependency} = { version = ${JSON.stringify(declaration.req)}, default-features = ${declaration.uses_default_features !== false}, features = ${JSON.stringify(declaration.features ?? [])} }`;
    });
    writeFileSync(
      join(dir, "Cargo.toml"),
      `[package]\nname = "isolated-${name}-consumer"\nversion = "0.0.0"\nedition = "2021"\n[workspace]\n[dependencies]\n${[...localDependencies, ...registryDependencies].join("\n")}\n`,
    );
    copyFileSync(
      join(crate, "examples", spec.example),
      join(dir, "src/main.rs"),
    );
    const args = spec.fixtures.map((fixture, index) => {
      const destination = join(dir, `input-${index}`);
      copyFileSync(join(root, fixture), destination);
      return destination;
    });
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
    if (resolve(metadata.workspace_root) !== dir)
      throw new ConsumerFailure("workspace-isolation-drift");
    if (resolve(metadata.target_directory) !== env.CARGO_TARGET_DIR)
      throw new ConsumerFailure("target-isolation-drift");
    for (const pkg of metadata.packages) {
      if (
        pkg.source === null &&
        !(
          pkg.name === `isolated-${name}-consumer` &&
          pkg.manifest_path === join(dir, "Cargo.toml")
        ) &&
        !spec.locals.some(
          (local) =>
            pkg.name === local &&
            pkg.manifest_path === join(root, "crates", local, "Cargo.toml"),
        )
      )
        throw new ConsumerFailure("unexpected-local-dependency", pkg.name);
      if (pkg.source !== null && !pkg.source.startsWith("registry+"))
        throw new ConsumerFailure("non-registry-dependency", pkg.name);
      if (
        /tauri|sqlx|sqlite|prmonitor|^ai-(codex|claude|cursor)$/.test(pkg.name)
      )
        throw new ConsumerFailure("unexpected-runtime-dependency", pkg.name);
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
  } catch (error) {
    receipt.passed = false;
    receipt.failure ??= {
      stage: receipt.stage,
      code:
        error instanceof ConsumerFailure ? error.code : "consumer-check-failed",
      ...(error instanceof ConsumerFailure &&
      /^[A-Za-z0-9_-]{1,64}$/.test(error.packageName ?? "")
        ? { package: error.packageName }
        : {}),
    };
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
export function checkRustConsumers(root, execute = spawnSync) {
  root = realpathSync(root);
  const reportDir = join(root, ".local-ci-runs");
  const reportPath = join(reportDir, "rust-consumers.json");
  mkdirSync(reportDir, { recursive: true });
  // Invalidate prior success even if writing this run's report later fails.
  rmSync(reportPath, { force: true });
  const report = {
    status: "running",
    startedAt: new Date().toISOString(),
    source: null,
    consumers: [],
  };
  const temporary = join(reportDir, `rust-consumers-${process.pid}.tmp`);
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
    for (const spec of rustConsumers) {
      const receipt = { name: spec.name, stage: "prepare", passed: false };
      report.consumers.push(receipt);
      checkOne(root, spec, owner, execute, receipt);
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
  const report = checkRustConsumers(
    fileURLToPath(new URL("../", import.meta.url)),
  );
  console.log(`Isolated rust consumers: ${report.status}`);
  for (const receipt of report.consumers.filter((r) => !r.passed)) {
    const failure = receipt.failure;
    console.error(
      `${receipt.name}: ${failure?.stage ?? receipt.stage}: ${failure?.code ?? "cargo-step-failed"}${failure?.package ? ` (${failure.package})` : ""}`,
    );
  }
  process.exitCode = report.status === "passed" ? 0 : 1;
}
