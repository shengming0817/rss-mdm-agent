import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
export const sha256 = (value) =>
  createHash("sha256").update(value).digest("hex");
export function sourceEvidence(root) {
  const git = (...args) =>
    execFileSync("/usr/bin/git", args, { cwd: root, encoding: "utf8" }).trim();
  const files = [
    ...new Set(
      git("ls-files", "-z", "--cached", "--others", "--exclude-standard")
        .split("\0")
        .filter(Boolean),
    ),
  ].sort();
  const hash = createHash("sha256");
  for (const file of files) {
    hash.update(file + "\0");
    try {
      hash.update(readFileSync(join(root, file)));
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      hash.update("<deleted>");
    }
  }
  return {
    head: git("rev-parse", "HEAD"),
    sourceSha256: hash.digest("hex"),
    cargoLockSha256: sha256(readFileSync(join(root, "Cargo.lock"))),
    pnpmLockSha256: sha256(readFileSync(join(root, "pnpm-lock.yaml"))),
    node: process.version,
    pnpm: execFileSync("pnpm", ["--version"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
  };
}
