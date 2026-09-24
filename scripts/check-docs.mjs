import { execFileSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const git = process.platform === "win32" ? "git" : "/usr/bin/git";
const files = [
  ...new Set(
    execFileSync(
      git,
      ["ls-files", "--cached", "--others", "--exclude-standard"],
      { cwd: root, encoding: "utf8" },
    )
      .trim()
      .split("\n"),
  ),
];
const errors = [];
for (const file of files.filter(
  (f) => f.endsWith(".md") && existsSync(resolve(root, f)),
)) {
  const source = readFileSync(resolve(root, file), "utf8");
  for (const match of source.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1].split("#")[0];
    if (!target || /^[a-z]+:/i.test(target)) continue;
    if (!existsSync(resolve(root, dirname(file), decodeURIComponent(target))))
      errors.push(`${file}: broken link ${target}`);
  }
  if (/\/Users\/|desktop\/packages\/ui|desktop\/apps\/client/.test(source))
    errors.push(`${file}: stale directory or personal path`);
}
if (
  readFileSync(resolve(root, "LICENSE"), "utf8") !==
  readFileSync(resolve(root, "packages/ui/LICENSE"), "utf8")
)
  errors.push("UI MIT license differs from root");
// Baseline lookup belongs to impact selection; whitespace checks also work before commit.
let base;
try {
  base = execFileSync(
    git,
    ["merge-base", process.env.CI_BASE || "origin/develop", "HEAD"],
    { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  ).trim();
} catch {
  /* A missing comparison ref does not prevent local documentation checks. */
}
if (base)
  execFileSync(git, ["diff", "--check", base], { cwd: root, stdio: "inherit" });
execFileSync(git, ["diff", "--cached", "--check"], {
  cwd: root,
  stdio: "inherit",
});
execFileSync(git, ["diff", "--check"], { cwd: root, stdio: "inherit" });
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else
  console.log(
    `Docs: local links and current paths checked (${files.filter((f) => f.endsWith(".md") && existsSync(resolve(root, f))).length} files)`,
  );
