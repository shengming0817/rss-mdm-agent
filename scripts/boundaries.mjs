import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse, compileTemplate } from "vue/compiler-sfc";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
// Medium: parse UI and desktop source, including Vue template expressions.
// The packed consumer test also verifies the downstream exported dependency closure.
export function checkSource(file, source) {
  file = file.replaceAll("\\", "/");
  const errors = [];
  if (
    /\b(prNumber|pr_number|repository|PullRequestView|usePrStore|useReviewStore)\b/.test(
      source,
    )
  )
    errors.push(`${file}: business coupling`);
  if (/\bv-html\s*=|\binnerHTML\b|\bouterHTML\b/.test(source))
    errors.push(`${file}: executable HTML sink`);
  const desktop = file.startsWith("apps/desktop/src/");
  const sourceDir = desktop ? "apps/desktop/src" : "packages/ui/src";
  const allowed = desktop
    ? ["vue", "@rss-mdm-agent/ui", "@rss-mdm-agent/ui/style.css"]
    : ["vue"];
  const descriptor = extname(file) === ".vue" ? parse(source).descriptor : null;
  const scripts = descriptor
    ? [
        descriptor.script?.content,
        descriptor.scriptSetup?.content,
        descriptor.template &&
          compileTemplate({
            source: descriptor.template.content,
            filename: file,
            id: "boundary",
          }).code,
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
      if (!allowed.includes(name) && !name.startsWith("."))
        errors.push(`${file}: unexpected dependency ${name}`);
      if (/(?:^|\/)(api|transport|stores?)(?:[./]|$)/.test(name))
        errors.push(`${file}: host dependency ${name}`);
      if (
        name.startsWith(".") &&
        !resolve(dirname(resolve(root, file)), name)
          .replaceAll("\\", "/")
          .startsWith(resolve(root, sourceDir).replaceAll("\\", "/") + "/")
      )
        errors.push(`${file}: import escapes presentation source`);
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
      const member = ts.isPropertyAccessExpression(node)
        ? node.name.text
        : ts.isElementAccessExpression(node) &&
            node.argumentExpression &&
            ts.isStringLiteral(node.argumentExpression)
          ? node.argumentExpression.text
          : null;
      if (
        [
          "innerHTML",
          "outerHTML",
          "insertAdjacentHTML",
          "parseFromString",
          "createContextualFragment",
          "setHTMLUnsafe",
          "write",
          "writeln",
        ].includes(member)
      )
        errors.push(`${file}: executable HTML sink ${member}`);
      if (
        ts.isIdentifier(node) &&
        [
          "DOMParser",
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
export function checkTree(treeRoot = root) {
  const read = (file) => readFileSync(resolve(treeRoot, file), "utf8");
  const errors = ["packages/ui/src", "apps/desktop/src"]
    .flatMap((dir) => files(resolve(treeRoot, dir)))
    .filter(
      (f) =>
        /\.(?:[cm]?[jt]sx?|vue)$/.test(f) &&
        !/\.(?:test|spec)\.[jt]sx?$/.test(f),
    )
    .flatMap((file) =>
      checkSource(relative(treeRoot, file), readFileSync(file, "utf8")),
    );
  const pkg = JSON.parse(read("packages/ui/package.json"));
  if (
    Object.keys(pkg.dependencies ?? {}).length ||
    Object.keys(pkg.optionalDependencies ?? {}).length ||
    Object.keys(pkg.peerDependencies ?? {}).join() !== "vue"
  )
    errors.push("UI production dependencies must be Vue only");
  const desktop = JSON.parse(read("apps/desktop/package.json"));
  if (
    Object.keys(desktop.dependencies ?? {})
      .sort()
      .join() !== "@rss-mdm-agent/ui,vue" ||
    Object.keys(desktop.optionalDependencies ?? {}).length ||
    Object.keys(desktop.peerDependencies ?? {}).length
  )
    errors.push("desktop production dependencies must be UI and Vue only");
  const config = JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json"));
  const capabilityFiles = files(
    resolve(treeRoot, "apps/desktop/src-tauri/capabilities"),
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
  for (const file of files(resolve(treeRoot, "apps/desktop/src-tauri/src"))) {
    if (
      /tauri::command|invoke_handler|\.plugin\s*\(|\.manage\s*\(|\.setup\s*\(|Command::new|prmonitor_lib/.test(
        readFileSync(file, "utf8"),
      )
    )
      errors.push(`${file}: business or background host registration`);
  }
  return errors;
}
