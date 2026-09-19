import { format } from "prettier";
import ts from "typescript";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const require = createRequire(
  new URL("../packages/ai-contract/package.json", import.meta.url),
);
const { compile } = require("json-schema-to-typescript");
const upstreamDirectory = new URL(
  "../packages/ai-contract/schema/upstream/a2ui/",
  import.meta.url,
);
const provenance = JSON.parse(
  readFileSync(new URL("manifest.json", upstreamDirectory), "utf8"),
);
for (const [name, digest] of Object.entries(provenance.sha256)) {
  const actual = createHash("sha256")
    .update(readFileSync(new URL(name, upstreamDirectory)))
    .digest("hex");
  if (actual !== digest) throw new Error(`Upstream A2UI source drift: ${name}`);
}
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
// The compiler omits unreachable $defs on a union root. Compile the capability
// metadata from the same source and append declarations not already emitted.
const metadata = await compile(
  {
    $schema: tsSchema.$schema,
    $defs: tsSchema.$defs,
    $ref: "#/$defs/Negotiation",
  },
  "Negotiation",
  { format: false, bannerComment: "" },
);
const emitted = new Set(
  ts
    .createSourceFile("records.ts", typescript, ts.ScriptTarget.Latest, true)
    .statements.filter((node) => node.name)
    .map((node) => node.name.text),
);
const metadataSource = ts.createSourceFile(
  "metadata.ts",
  metadata,
  ts.ScriptTarget.Latest,
  true,
);
typescript +=
  "\n" +
  metadataSource.statements
    .filter((node) => node.name && !emitted.has(node.name.text))
    .map((node) => node.getFullText(metadataSource))
    .join("\n");
const source = ts.createSourceFile(
  "wire.ts",
  typescript,
  ts.ScriptTarget.Latest,
  true,
);
const edits = [];
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
const catalogIdentity = Object.fromEntries(
  Object.entries(schema.$defs.A2uiNegotiation.properties).map(
    ([key, field]) => {
      if (typeof field.const !== "string")
        throw new Error(`Missing canonical catalog constant: ${key}`);
      return [key, field.const];
    },
  ),
);
outputs.set(
  "packages/ai-contract/src/identity.ts",
  await format(
    "// @generated from schema/runtime.schema.json. Do not edit.\n" +
      `export const interactionCatalog = Object.freeze(${JSON.stringify(catalogIdentity)} as const);\n` +
      `export const errorCodes = Object.freeze(${JSON.stringify(schema.$defs.ErrorCode.enum)} as const);\n`,
    { parser: "typescript" },
  ),
);
const { Ajv2020 } = require("ajv/dist/2020.js");
const standalone = require("ajv/dist/standalone/index.js").default;
const compileValidator = (schema, strict, referenced = []) => {
  const ajv = new Ajv2020({
    strict,
    validateFormats: false,
    code: { source: true, esm: true },
  });
  for (const [schema, key] of referenced) ajv.addSchema(schema, key);
  const validate = ajv.compile(schema);
  let code = standalone(ajv, validate);
  // Ajv's standalone helpers use CommonJS expressions even in ESM output.
  const imports = new Map();
  code = code.replace(
    /require\("(ajv\/dist\/runtime\/[^" ]+)"\)\.default/g,
    (_match, path) => {
      if (!imports.has(path)) imports.set(path, `helper${imports.size}`);
      return imports.get(path);
    },
  );
  return (
    "// @ts-nocheck\n// @generated from canonical schemas; do not edit.\n" +
    [...imports]
      .map(
        ([path, name]) =>
          `const ${name} = ${require(path).default.toString()};`,
      )
      .join("\n") +
    "\n" +
    code
  );
};
outputs.set(
  "packages/ai-contract/src/validate-record.ts",
  await format(compileValidator(schema, true), { parser: "typescript" }),
);
outputs.set(
  "packages/ai-contract/src/validate-negotiation.ts",
  await format(
    compileValidator(
      {
        $schema: schema.$schema,
        $defs: schema.$defs,
        $ref: "#/$defs/Negotiation",
      },
      true,
    ),
    { parser: "typescript" },
  ),
);
const actionSchema = JSON.parse(
  readFileSync(
    new URL(
      "../packages/ai-contract/schema/upstream/a2ui/client_to_server.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
outputs.set(
  "packages/ai-contract/src/validate-action.ts",
  await format(compileValidator(actionSchema, false), { parser: "typescript" }),
);
const upstream = (name) =>
  JSON.parse(
    readFileSync(
      new URL(
        `../packages/ai-contract/schema/upstream/a2ui/${name}.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  );
outputs.set(
  "packages/ai-contract/src/validate-surface.ts",
  await format(
    compileValidator(upstream("server_to_client"), false, [
      [upstream("common_types")],
      [upstream("catalog"), "https://a2ui.org/specification/v0_9/catalog.json"],
    ]),
    { parser: "typescript" },
  ),
);

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
