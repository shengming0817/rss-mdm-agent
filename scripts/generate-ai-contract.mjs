import { format } from "prettier";
import ts from "typescript";
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
// json-schema-to-typescript inlines $ref siblings with descriptions, duplicating
// named types. Compile references intact, then attach those schema-owned member
// docs to the generated TS AST. This is a compiler projection, not another model.
const tsSchema = structuredClone(schema),
  memberDocs = new Map();
function project(node, owner) {
  if (!node || typeof node !== "object") return;
  for (const [name, property] of Object.entries(node.properties ?? {})) {
    if (property.$ref && property.description)
      memberDocs.set(`${owner}.${name}`, property.description);
  }
  if (node.$ref) delete node.description;
  for (const value of Object.values(node)) project(value, owner);
}
for (const [owner, definition] of Object.entries(tsSchema.$defs))
  project(definition, owner);
let typescript = await compile(tsSchema, "WireRecord", {
  format: false,
  additionalProperties: false,
  unreachableDefinitions: true,
  bannerComment: "// @generated from schema/runtime.schema.json. Do not edit.",
});
const source = ts.createSourceFile(
  "wire.ts",
  typescript,
  ts.ScriptTarget.Latest,
  true,
);
const edits = [];
// Closed schema alternatives also forbid fields belonging only to another branch.
// Express absence as optional never so property access remains ergonomic without
// weakening the generated discriminated union or accepting legacy wire fields.
function closeUnions(node) {
  if (ts.isUnionTypeNode(node) && node.types.every(ts.isTypeLiteralNode)) {
    const keys = new Set(
      node.types.flatMap((t) =>
        t.members
          .filter(ts.isPropertySignature)
          .map((m) => m.name.getText(source)),
      ),
    );
    for (const member of node.types) {
      const own = new Set(
        member.members
          .filter(ts.isPropertySignature)
          .map((m) => m.name.getText(source)),
      );
      const missing = [...keys].filter((key) => !own.has(key));
      if (missing.length)
        edits.push([
          member.end - 1,
          missing.map((key) => `${key}?: never;`).join("\n"),
        ]);
    }
  }
  ts.forEachChild(node, closeUnions);
}
closeUnions(source);
function document(node, owner) {
  if (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node))
    owner = node.name.text;
  if (ts.isPropertySignature(node)) {
    const description = memberDocs.get(`${owner}.${node.name.getText(source)}`);
    if (description)
      edits.push([
        node.getStart(source),
        `/** ${description.replaceAll("*/", "* / ")} */\n`,
      ]);
  }
  ts.forEachChild(node, (child) => document(child, owner));
}
document(source, "");
for (const [offset, text] of edits.sort((a, b) => b[0] - a[0]))
  typescript = typescript.slice(0, offset) + text + typescript.slice(offset);
const outputs = new Map();
outputs.set(
  "packages/ai-contract/src/wire.ts",
  await format(typescript, { parser: "typescript" }),
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
