import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, dirname, resolve } from "node:path";
import ts from "typescript";
const root = fileURLToPath(new URL("../", import.meta.url));
const appExternalDependencies = {
  "@modelcontextprotocol/sdk": "1.30.0",
  yaml: "2.9.1",
};
const allowed = {
  "apps/ai-host": [
    "@rss-mdm-agent/ai-contract",
    "@rss-mdm-agent/ai-host",
    "@rss-mdm-agent/ai-store-sqlite",
    "@rss-mdm-agent/ai-access",
    "@rss-mdm-agent/ai-adapter-claude",
    "@rss-mdm-agent/ai-adapter-codex",
    "@rss-mdm-agent/ai-adapter-deepseek",
    ...Object.keys(appExternalDependencies),
  ],
  "packages/ai-host": ["@rss-mdm-agent/ai-contract"],
  "packages/ai-contract": ["@noble/hashes", "canonicalize", "jsonc-parser"],
  "packages/ai-access": [
    "@rss-mdm-agent/ai-contract",
    "@agentclientprotocol/sdk",
  ],
  "packages/ai-client": [
    "@rss-mdm-agent/ai-contract",
    "@agentclientprotocol/sdk",
  ],
  "packages/ai-ui-bridge": [
    "@rss-mdm-agent/ai-contract",
    "@rss-mdm-agent/ai-client",
    "@a2ui/lit",
    "@a2ui/web_core",
    "vue",
  ],
};
const errors = [];
const serverFiles = new Map([
  [
    join(root, "packages/ai-contract/src/session.ts"),
    ["node:path", "node:crypto"],
  ],
  [join(root, "packages/ai-contract/src/transitions.ts"), ["node:crypto"]],
]);
for (const [file, imports] of Object.entries({
  "index.ts": ["node:crypto", "node:util"],
  "channel.ts": ["node:crypto", "node:stream"],
  "delivery.ts": ["node:crypto"],
  "bootstrap.ts": ["node:net", "node:child_process"],
  "process.ts": [
    "node:child_process",
    "node:crypto",
    "node:stream",
    "node:url",
  ],
}))
  serverFiles.set(join(root, "packages/ai-host/src", file), imports);
for (const [file, imports] of Object.entries({
  "index.ts": ["node:stream", "node:fs/promises", "node:path"],
  "configuration.ts": ["node:path", "node:crypto"],
  "native.ts": ["node:net", "node:stream", "node:string_decoder"],
  "secrets.ts": ["node:crypto"],
  "resolver.ts": ["node:crypto", "node:fs/promises", "node:path"],
  "connection.ts": ["node:os", "node:path"],
  "execution.ts": ["node:stream", "node:crypto"],
  "provider.ts": [
    "node:crypto",
    "node:dns/promises",
    "node:fs/promises",
    "node:path",
  ],
  "private-file.ts": ["node:fs", "node:fs/promises", "node:path"],
}))
  serverFiles.set(join(root, "apps/ai-host/src", file), imports);
const runtimeEdges = new Map();
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
    (name === "apps/ai-host" &&
      Object.entries(manifest.dependencies ?? {}).some(
        ([dependency, value]) =>
          value !==
          (dependency.startsWith("@rss-mdm-agent/")
            ? "workspace:*"
            : appExternalDependencies[dependency]),
      )) ||
    manifest.optionalDependencies ||
    manifest.peerDependencies
  )
    errors.push(`${name}: unexpected dependency closure`);
  for (const file of walk(join(dir, "src"))) {
    if (!file.endsWith(".ts")) continue;
    const source = readFileSync(file, "utf8"),
      ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
    const check = (value, runtime = true) => {
      if (value.startsWith(".")) {
        const target = resolve(dirname(file), value.replace(/\.js$/, ".ts"));
        if (!target.startsWith(join(dir, "src") + "/"))
          errors.push(`${name}: relative import escapes package`);
        if (runtime) {
          const edges = runtimeEdges.get(file) ?? [];
          edges.push(resolve(dirname(file), value.replace(/\.js$/, ".ts")));
          runtimeEdges.set(file, edges);
        }
        return;
      }
      if (
        name !== "packages/ai-contract" &&
        name !== "packages/ai-host" &&
        name !== "apps/ai-host" &&
        [
          "@rss-mdm-agent/ai-contract/session",
          "@rss-mdm-agent/ai-contract/transitions",
          "@rss-mdm-agent/ai-contract/testing",
        ].includes(value) &&
        runtime
      )
        errors.push(`${name}: server entry in portable package`);
      if (serverFiles.get(file)?.includes(value)) return;
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
        check(
          node.moduleSpecifier.text,
          ts.isExportDeclaration(node)
            ? !node.isTypeOnly &&
                !(
                  node.exportClause &&
                  ts.isNamedExports(node.exportClause) &&
                  node.exportClause.elements.every((e) => e.isTypeOnly)
                )
            : !node.importClause?.isTypeOnly &&
                !(
                  node.importClause?.namedBindings &&
                  ts.isNamedImports(node.importClause.namedBindings) &&
                  !node.importClause.name &&
                  node.importClause.namedBindings.elements.every(
                    (e) => e.isTypeOnly,
                  )
                ),
        );
      if (
        ts.isCallExpression(node) &&
        ["require", "import"].includes(node.expression.getText(ast))
      ) {
        if (ts.isStringLiteralLike(node.arguments[0]))
          check(node.arguments[0].text);
        else if (
          !(
            file === join(root, "packages/ai-host/src/bootstrap.ts") &&
            node.expression.getText(ast) === "import" &&
            node.arguments[0]?.getText(ast) === "input.artifact"
          )
        )
          errors.push(`${name}: computed module import`);
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
const visited = new Set();
function portable(file) {
  if (visited.has(file)) return;
  visited.add(file);
  if (serverFiles.has(file))
    errors.push("ai-contract: browser entry reaches server runtime");
  for (const next of runtimeEdges.get(file) ?? []) portable(next);
}
portable(join(root, "packages/ai-contract/src/index.ts"));
if (errors.length) throw new Error(errors.join("\n"));
console.log(
  "PASS AI package boundaries; renderer browser closure is additionally built and executed by the isolated consumer",
);
