// Run only on the administrator-prepared laboratory machine. Missing setup fails.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { sourceState } from "./source-state.mjs";
const executable = process.argv[2] && resolve(process.argv[2]);
if (!executable)
  throw new Error("Pass the installed fixed desktop executable.");
const result = spawnSync(executable, ["--service-probe"], {
  encoding: "utf8",
  timeout: 10000,
});
let view;
try {
  view = JSON.parse(result.stdout);
} catch {}
const passed =
  result.status === 0 &&
  view?.phase === "connected" &&
  view.status.capability === "statusOnly";
mkdirSync(".local-ci-runs", { recursive: true });
writeFileSync(
  ".local-ci-runs/service-platform.json",
  JSON.stringify(
    {
      source: sourceState(process.cwd()),
      platform: process.platform,
      arch: process.arch,
      artifactSha256: createHash("sha256")
        .update(readFileSync(executable))
        .digest("hex"),
      at: new Date().toISOString(),
      command: ["<installed-desktop>", "--service-probe"],
      result: {
        exitCode: result.status,
        stdout: result.stdout,
        stderr: result.stderr,
      },
      status: passed ? "passed-status-query-only" : "failed",
      notCovered: [
        "hostile-process",
        "cross-user",
        "tamper",
        "replay",
        "revocation",
        "restart",
        "real-desktop-codex",
      ],
    },
    null,
    2,
  ),
);
if (!passed)
  throw new Error("Real service query failed; see local platform receipt.");
