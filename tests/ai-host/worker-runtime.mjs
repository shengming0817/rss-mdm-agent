import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
const root = new URL("../../.local-ci-runs/worker-runtime/", import.meta.url);
export const workerRuntime = {
  launcher: fileURLToPath(
    new URL(
      "bin/rss-ai-worker-launcher" +
        (process.platform === "win32" ? ".exe" : ""),
      root,
    ),
  ),
  manifestDigest: createHash("sha256")
    .update(readFileSync(new URL("worker-manifest.json", root)))
    .digest("hex"),
};
