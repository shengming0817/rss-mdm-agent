import { spawnSync } from "node:child_process";
import {
  cpSync,
  copyFileSync,
  chmodSync,
  mkdirSync,
  writeFileSync,
  symlinkSync,
  existsSync,
} from "node:fs";
import { resolve, join } from "node:path";
import { cargoTargetDir } from "./cargo-target.mjs";
import { stageWorkerRuntime } from "./ai-host-artifacts.mjs";
const build = spawnSync(
  "cargo",
  [
    "build",
    "--release",
    "--locked",
    "-p",
    "native-process",
    "-p",
    "platform-private-storage",
  ],
  { stdio: "inherit" },
);
if (build.status !== 0) throw new Error("native worker launcher build failed");
const root = resolve(".local-ci-runs/worker-runtime");
mkdirSync(join(root, "bin"), { recursive: true });
if (process.argv.includes("--private-only")) {
  const suffix = process.platform === "win32" ? ".exe" : "";
  copyFileSync(process.execPath, join(root, "bin", "node" + suffix));
  const helper = join(root, "bin", "rss-private-storage" + suffix);
  copyFileSync(
    join(
      cargoTargetDir(resolve(".")),
      "release",
      "rss-private-storage" + suffix,
    ),
    helper,
  );
  if (process.platform !== "win32") chmodSync(helper, 0o755);
  process.exit(0);
}

cpSync("packages/ai-host/dist", join(root, "worker"), { recursive: true });
writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
if (!existsSync(join(root, "node_modules")))
  symlinkSync(
    resolve("packages/ai-host/node_modules"),
    join(root, "node_modules"),
    process.platform === "win32" ? "junction" : "dir",
  );
stageWorkerRuntime(resolve("."), root, process.execPath, "worker/bootstrap.js");
