import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse } from "vue/compiler-sfc";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(resolve(root, file), "utf8");
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
// Medium: parse every UI import/export and call, not a hand-maintained callsite list.
// The packed consumer test also verifies the downstream exported dependency closure.
export function checkSource(file, source) {
  const errors = [];
  if (
    /\b(prNumber|pr_number|repository|PullRequestView|usePrStore|useReviewStore)\b/.test(
      source,
    )
  )
    errors.push(`${file}: business coupling`);
  if (/\bv-html\s*=|\binnerHTML\b|\bouterHTML\b/.test(source))
    errors.push(`${file}: executable HTML sink`);
  const scripts =
    extname(file) === ".vue"
      ? [
          parse(source).descriptor.script?.content,
          parse(source).descriptor.scriptSetup?.content,
        ].filter(Boolean)
      : [source];
  for (const script of scripts) {
    const ast = ts.createSourceFile(
      file + ".ts",
      script,
      ts.ScriptTarget.Latest,
      true,
    );
    const checkImport = (name) => {
      if (name !== "vue" && !name.startsWith("."))
        errors.push(`${file}: unexpected dependency ${name}`);
      if (/(?:^|\/)(api|transport|stores?)(?:\/|$)/.test(name))
        errors.push(`${file}: host dependency ${name}`);
      if (
        name.startsWith(".") &&
        !resolve(dirname(resolve(root, file)), name).startsWith(
          resolve(root, "packages/ui/src") + "/",
        )
      )
        errors.push(`${file}: import escapes UI package`);
    };
    function visit(node) {
      if (
        (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier &&
        ts.isStringLiteral(node.moduleSpecifier)
      )
        checkImport(node.moduleSpecifier.text);
      if (ts.isCallExpression(node)) {
        if (
          node.expression.kind === ts.SyntaxKind.ImportKeyword ||
          node.expression.getText(ast) === "require"
        ) {
          errors.push(
            `${file}: dynamic module loading is not a presentation dependency`,
          );
        }
      }
      if (
        ts.isIdentifier(node) &&
        [
          "fetch",
          "WebSocket",
          "XMLHttpRequest",
          "EventSource",
          "eval",
          "Function",
          "localStorage",
          "sessionStorage",
        ].includes(node.text)
      )
        errors.push(`${file}: non-presentation capability ${node.text}`);
      ts.forEachChild(node, visit);
    }
    visit(ast);
  }
  return errors;
}
export function checkTree() {
  const errors = files(resolve(root, "packages/ui/src"))
    .filter((f) => /\.(ts|vue)$/.test(f))
    .flatMap((file) =>
      checkSource(relative(root, file), readFileSync(file, "utf8")),
    );
  const pkg = JSON.parse(read("packages/ui/package.json"));
  if (
    Object.keys(pkg.dependencies ?? {}).length ||
    Object.keys(pkg.peerDependencies ?? {}).join() !== "vue"
  )
    errors.push("UI production dependencies must be Vue only");
  const config = JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json"));
  const capabilityFiles = files(
    resolve(root, "apps/desktop/src-tauri/capabilities"),
  );
  if (capabilityFiles.length !== 1) errors.push("unexpected capabilities");
  for (const file of capabilityFiles) {
    const cap = JSON.parse(readFileSync(file, "utf8"));
    if (
      JSON.stringify(cap.windows) !== '["main"]' ||
      cap.permissions?.length !== 0 ||
      cap.remote
    )
      errors.push("host capability must be local main with zero permissions");
  }
  if (
    JSON.stringify(config.app.security.capabilities) !== '["main"]' ||
    config.app.withGlobalTauri ||
    Object.keys(config.plugins ?? {}).length
  )
    errors.push("unexpected host registration");
  if (
    !config.app.security.csp ||
    !config.app.security.devCsp ||
    config.app.security.csp.includes("unsafe-eval") ||
    config.app.security.csp.includes("ws:")
  )
    errors.push("CSP must restrict production resources");
  const cargo = read("apps/desktop/src-tauri/Cargo.toml");
  if (/tauri-plugin|prmonitor|rusqlite|reqwest|tokio|portable-pty/.test(cargo))
    errors.push("unexpected host dependency");
  for (const file of files(resolve(root, "apps/desktop/src-tauri/src"))) {
    if (
      /tauri::command|invoke_handler|\.plugin\s*\(|\.manage\s*\(|\.setup\s*\(|Command::new|prmonitor_lib/.test(
        readFileSync(file, "utf8"),
      )
    )
      errors.push(`${file}: business or background host registration`);
  }
  return errors;
}
