import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { sourceState } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
function run(args, cwd, env, capture = false) {
  const result = spawnSync("cargo", args, {
    cwd,
    env,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.status !== 0 || result.error || result.signal)
    throw new Error(
      `cargo ${args[0]} failed: ${result.error ?? result.signal ?? result.status}`,
    );
  return result.stdout;
}
const owner = JSON.parse(
  run(
    ["metadata", "--no-deps", "--locked", "--format-version", "1"],
    root,
    process.env,
    true,
  ),
);
const reports = [];
for (const name of ["execution-contract", "ai-session-contract"]) {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), `${name}-consumer-`)));
  const crate = join(root, "crates", name);
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: join(dir, "target"),
    RUSTFLAGS: "",
    CARGO_ENCODED_RUSTFLAGS: "",
    RUSTC_WRAPPER: "",
    RUSTC_WORKSPACE_WRAPPER: "",
  };
  try {
    mkdirSync(join(dir, "src"));
    const serdeVersion = owner.packages
      .find((p) => p.name === name)
      .dependencies.find((d) => d.name === "serde_json").req;
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
    run(["generate-lockfile", "--offline"], dir, env);
    const metadata = JSON.parse(
      run(
        ["metadata", "--offline", "--locked", "--format-version", "1"],
        dir,
        env,
        true,
      ),
    );
    if (
      resolve(metadata.workspace_root) !== resolve(dir) ||
      resolve(metadata.target_directory) !== resolve(env.CARGO_TARGET_DIR)
    )
      throw new Error("consumer workspace/target isolation failed");
    for (const pkg of metadata.packages) {
      if (
        pkg.source === null &&
        ![join(dir, "Cargo.toml"), join(crate, "Cargo.toml")].includes(
          pkg.manifest_path,
        )
      )
        throw new Error(`unexpected source dependency: ${pkg.name}`);
      if (pkg.source !== null && !pkg.source.startsWith("registry+"))
        throw new Error(`non-registry dependency: ${pkg.name}`);
      if (
        /tauri|sqlx|sqlite|prmonitor|^ai-(codex|claude|cursor)$/.test(pkg.name)
      )
        throw new Error(`unexpected runtime dependency: ${pkg.name}`);
    }
    run(["run", "--offline", "--locked", "--", ...args], dir, env);
    reports.push({
      name,
      source: sourceState(root),
      lockSha256: createHash("sha256")
        .update(readFileSync(join(dir, "Cargo.lock")))
        .digest("hex"),
      dependencies: metadata.packages.map((p) => ({
        name: p.name,
        version: p.version,
        source: p.source,
      })),
      command: "cargo run --offline --locked -- <fixture>",
      passed: true,
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
writeFileSync(
  join(root, ".local-ci-runs/contracts.json"),
  JSON.stringify(reports, null, 2),
);
console.log(
  "Both isolated Rust consumers ran successfully with separate workspaces, locks and targets",
);
