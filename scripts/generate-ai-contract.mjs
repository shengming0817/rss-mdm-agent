import { format } from "prettier";
import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const require = createRequire(
  new URL("../packages/ai-contract/package.json", import.meta.url),
);
const { compile } = require("json-schema-to-typescript");
const schemaPath = "packages/ai-contract/schema/runtime.schema.json";
const schemaText = readFileSync(
  new URL("../" + schemaPath, import.meta.url),
  "utf8",
);
const schema = JSON.parse(schemaText);
const outputs = new Map();
outputs.set(
  "packages/ai-contract/src/wire.ts",
  await format(
    await compile(schema, "WireRecord", {
      format: false,
      additionalProperties: false,
      unreachableDefinitions: true,
      bannerComment:
        "// @generated from schema/runtime.schema.json. Do not edit.",
    }),
    { parser: "typescript" },
  ),
);
const rust = spawnSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "ai-session-contract",
    "--example",
    "generate-runtime",
    "--",
    schemaPath,
  ],
  { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
);
if (rust.status !== 0) {
  process.stderr.write(rust.stderr ?? "");
  process.exit(rust.status ?? 1);
}
const formatted = spawnSync("rustfmt", ["--edition", "2021"], {
  input: rust.stdout,
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024,
});
if (formatted.status !== 0)
  throw new Error("Rust generation formatting failed");
outputs.set("crates/ai-session-contract/src/generated.rs", formatted.stdout);
outputs.set("crates/ai-session-contract/src/schema.json", schemaText);
for (const [path, content] of outputs) {
  const url = new URL("../" + path, import.meta.url);
  if (process.argv.includes("--check")) {
    if (readFileSync(url, "utf8") !== content)
      throw new Error("Generated artifact drift: " + path);
  } else writeFileSync(url, content);
}
console.log(
  "AI contract: Rust/TS bindings and schema projection match their single source",
);
