import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import ts from "typescript";
const root = fileURLToPath(new URL("../packages/", import.meta.url));
const allowed = {
  "ai-contract": ["@noble/hashes", "canonicalize", "jsonc-parser"],
  "ai-access": ["@rss-mdm-agent/ai-contract", "@agentclientprotocol/sdk"],
  "ai-client": ["@rss-mdm-agent/ai-contract", "@agentclientprotocol/sdk"],
  "ai-ui-bridge": [
    "@rss-mdm-agent/ai-contract",
    "@rss-mdm-agent/ai-client",
    "@a2ui/lit",
    "@a2ui/web_core",
    "vue",
  ],
};
const errors = [];
function walk(path) {
  return readdirSync(path, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? e.name === "testing"
        ? []
        : walk(join(path, e.name))
      : [join(path, e.name)],
  );
}
for (const [name, dependencies] of Object.entries(allowed)) {
  const dir = join(root, name),
    manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  if (
    JSON.stringify(Object.keys(manifest.dependencies ?? {}).sort()) !==
      JSON.stringify(dependencies.toSorted()) ||
    manifest.optionalDependencies ||
    manifest.peerDependencies
  )
    errors.push(`${name}: unexpected dependency closure`);
  for (const file of walk(join(dir, "src"))) {
    if (!file.endsWith(".ts")) continue;
    const source = readFileSync(file, "utf8"),
      ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const check = (value) => {
      if (value.startsWith(".")) return;
      const packageName = value.startsWith("@")
        ? value.split("/").slice(0, 2).join("/")
        : value.split("/")[0];
      if (!dependencies.includes(packageName))
        errors.push(`${name}: forbidden import ${value}`);
    };
    function visit(node) {
      if (
        (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier &&
        ts.isStringLiteralLike(node.moduleSpecifier)
      )
        check(node.moduleSpecifier.text);
      if (
        ts.isCallExpression(node) &&
        ["require", "import"].includes(node.expression.getText(ast))
      ) {
        if (ts.isStringLiteralLike(node.arguments[0]))
          check(node.arguments[0].text);
        else errors.push(`${name}: computed module import`);
      }
      ts.forEachChild(node, visit);
    }
    visit(ast);
    if (
      /\binnerHTML\b|\bouterHTML\b|\bunsafeHTML\s*\(|\beval\s*\(|new\s+Function\s*\(/.test(
        source,
      )
    )
      errors.push(`${name}: executable content sink`);
  }
}
if (errors.length) throw new Error(errors.join("\n"));
console.log(
  "PASS AI package boundaries; renderer browser closure is additionally built and executed by the isolated consumer",
);
