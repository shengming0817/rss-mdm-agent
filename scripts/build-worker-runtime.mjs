import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  writeFileSync,
  symlinkSync,
  existsSync,
} from "node:fs";
import { resolve, join } from "node:path";
import { stageWorkerRuntime } from "./ai-host-artifacts.mjs";
const build = spawnSync(
  "cargo",
  ["build", "--release", "--locked", "-p", "native-process"],
  { stdio: "inherit" },
);
if (build.status !== 0) throw new Error("native worker launcher build failed");
const root = resolve(".local-ci-runs/worker-runtime");
mkdirSync(join(root, "bin"), { recursive: true });
cpSync("packages/ai-host/dist", join(root, "worker"), { recursive: true });
writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
if (!existsSync(join(root, "node_modules")))
  symlinkSync(
    resolve("packages/ai-host/node_modules"),
    join(root, "node_modules"),
    process.platform === "win32" ? "junction" : "dir",
  );
stageWorkerRuntime(resolve("."), root, process.execPath, "worker/bootstrap.js");
