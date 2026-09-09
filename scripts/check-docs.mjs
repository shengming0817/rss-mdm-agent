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
for (const file of files.filter((f) => f.endsWith(".md"))) {
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
execFileSync(git, ["diff", "--check"], { cwd: root, stdio: "inherit" });
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else
  console.log(
    `Docs: local links and current paths checked (${files.filter((f) => f.endsWith(".md")).length} files)`,
  );
