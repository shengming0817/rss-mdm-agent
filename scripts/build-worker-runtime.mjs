import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  symlinkSync,
  existsSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { resolve, join } from "node:path";
const build = spawnSync(
  "cargo",
  ["build", "--release", "--locked", "-p", "native-process"],
  { stdio: "inherit" },
);
if (build.status !== 0) throw new Error("native worker launcher build failed");
const root = resolve(".local-ci-runs/worker-runtime");
mkdirSync(join(root, "bin"), { recursive: true });
const suffix = process.platform === "win32" ? ".exe" : "";
copyFileSync(process.execPath, join(root, "bin", "node" + suffix));
copyFileSync(
  resolve("target/release/rss-ai-worker-launcher" + suffix),
  join(root, "bin", "rss-ai-worker-launcher" + suffix),
);
copyFileSync(
  resolve("target/release/rss-private-storage" + suffix),
  join(root, "bin", "rss-private-storage" + suffix),
);
cpSync("packages/ai-host/dist", join(root, "worker"), { recursive: true });
writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
if (!existsSync(join(root, "node_modules")))
  symlinkSync(
    resolve("packages/ai-host/node_modules"),
    join(root, "node_modules"),
    process.platform === "win32" ? "junction" : "dir",
  );
const hash = (path) =>
  createHash("sha256")
    .update(readFileSync(join(root, path)))
    .digest("hex");
writeFileSync(
  join(root, "worker-manifest.json"),
  JSON.stringify({
    version: 1,
    node: "bin/node" + suffix,
    bootstrap: "worker/bootstrap.js",
    node_sha256: hash("bin/node" + suffix),
    bootstrap_sha256: hash("worker/bootstrap.js"),
  }) + "\n",
);
