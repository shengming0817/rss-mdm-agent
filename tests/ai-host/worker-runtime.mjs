import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { preparedArtifact } from "../../scripts/preparation.mjs";
const root = preparedArtifact("worker");
export const workerRuntime = {
  launcher: join(
    root,
    "bin/rss-ai-worker-launcher" + (process.platform === "win32" ? ".exe" : ""),
  ),
  manifestDigest: createHash("sha256")
    .update(readFileSync(join(root, "worker-manifest.json")))
    .digest("hex"),
};
