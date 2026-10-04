// ref: typify src/lib.rs@0.8.0; Cargo reference/external-tools.md.
import { helperOutput } from "./preparation.mjs";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const directory = "crates/agent-api/";
const source = JSON.parse(
  readFileSync(directory + "schema/source.json", "utf8"),
);
for (const [path, digest] of Object.entries(source.sha256)) {
  const actual = createHash("sha256")
    .update(readFileSync(directory + path))
    .digest("hex");
  if (actual !== digest) throw Error(`Agent upstream schema changed: ${path}`);
}
for (const family of ["runtime", "enroll"]) {
  const generated = execFileSync(
    "rustfmt",
    ["--edition", "2021", "--emit", "stdout"],
    {
      cwd: root,
      input: await helperOutput("agentApi", [family]),
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    },
  );
  const path = `${directory}src/${family}.rs`;
  if (process.argv.includes("--check")) {
    if (readFileSync(path, "utf8") !== generated)
      throw Error(`Agent API binding drift: ${family}`);
  } else writeFileSync(path, generated);
}
console.log("Agent API bindings match the local upstream schema snapshot");
