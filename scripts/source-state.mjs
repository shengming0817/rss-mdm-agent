import { execFileSync } from "node:child_process";
export const git = process.platform === "win32" ? "git" : "/usr/bin/git";
export function sourceState(
  cwd,
  baseRef = process.env.CI_BASE || "origin/develop",
) {
  const run = (...args) =>
    execFileSync(git, args, { cwd, encoding: "utf8" }).trim();
  const head = run("rev-parse", "HEAD");
  const baseOid = run("rev-parse", "--verify", `${baseRef}^{commit}`);
  const base = run("merge-base", baseOid, head);
  const changes = run("status", "--porcelain", "--untracked-files=all");
  return { head, baseRef, baseOid, base, clean: changes === "", changes };
}

// Canonical verdict for one committed source identity across a verification run.
export function sameCommittedSource(start, end) {
  return (
    start.clean &&
    end.clean &&
    start.head === end.head &&
    start.base === end.base &&
    start.baseRef === end.baseRef &&
    typeof start.baseOid === "string" &&
    start.baseOid === end.baseOid
  );
}
