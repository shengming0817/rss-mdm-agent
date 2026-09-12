import { execFileSync } from "node:child_process";
export const git = process.platform === "win32" ? "git" : "/usr/bin/git";
export function sourceState(
  cwd,
  baseRef = process.env.CI_BASE || "origin/develop",
) {
  const run = (...args) =>
    execFileSync(git, args, { cwd, encoding: "utf8" }).trim();
  const head = run("rev-parse", "HEAD");
  const base = run("merge-base", baseRef, head);
  const changes = run("status", "--porcelain", "--untracked-files=all");
  return { head, baseRef, base, clean: changes === "", changes };
}

// Canonical verdict for one committed source identity across a verification run.
export function sameCommittedSource(start, end) {
  return (
    start.clean &&
    end.clean &&
    start.head === end.head &&
    start.base === end.base &&
    start.baseRef === end.baseRef
  );
}
