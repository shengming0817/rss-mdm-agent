import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse, compileTemplate } from "vue/compiler-sfc";

const fixtureCommands = [
  "self_service_snapshot",
  "self_service_preview",
  "self_service_submit",
  "self_service_respond",
];
const nativeAdapter = "apps/desktop/src/self-service/native.ts";
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
  if (file === nativeAdapter) allowed.push("@tauri-apps/api/core");
  const descriptor = extname(file) === ".vue" ? parse(source).descriptor : null;
  if (descriptor) {
    const template = descriptor.template?.content ?? "";
    // No resource/navigation attributes or dynamic tag/attribute injection in this sample.
    const templateAst = compileTemplate({
      source: template,
      filename: file,
      id: "boundary",
    });
    if (templateAst.errors.length) errors.push(`${file}: invalid template`);
    function visitTemplate(node) {
      if (node.type === 1) {
        if (
          [
            "a",
            "area",
            "iframe",
            "object",
            "embed",
            "script",
            "link",
            "base",
            "meta",
            "img",
            "video",
            "audio",
            "source",
            "component",
          ].includes(node.tag.toLowerCase())
        )
          errors.push(`${file}: non-presentation element ${node.tag}`);
        for (const prop of node.props) {
          const name =
            prop.type === 6
              ? prop.name
              : prop.arg?.isStatic
                ? prop.arg.content
                : null;
          if (
            [
              "href",
              "src",
              "srcset",
              "action",
              "formaction",
              "srcdoc",
              "ping",
              "is",
            ].includes(name?.toLowerCase()) ||
            (prop.type === 7 && ["bind", "on"].includes(prop.name) && !name)
          )
            errors.push(`${file}: outbound or dynamic template attribute`);
        }
        if (
          node.tag === "form" &&
          !node.props.some(
            (p) =>
              p.type === 7 &&
              p.name === "on" &&
              p.arg?.content === "submit" &&
              p.modifiers.some((m) => (m.content ?? m) === "prevent"),
          )
        )
          errors.push(`${file}: form must prevent native submission`);
      }
      for (const child of node.children ?? []) visitTemplate(child);
    }
    if (descriptor.template?.ast) visitTemplate(descriptor.template.ast);
  }
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
  for (const script of [scripts.join("\n")]) {
    const ast = ts.createSourceFile(
      file + ".ts",
      script,
      ts.ScriptTarget.Latest,
      true,
    );
    // Resolve lexical symbols instead of enumerating every browser global. Local aliases
    // remain checked at their source; ambient value access is a closed allowlist.
    const program = ts.createProgram(
      [ast.fileName],
      { noLib: true, noResolve: true },
      {
        ...ts.createCompilerHost({}),
        getSourceFile: (name) => (name === ast.fileName ? ast : undefined),
        writeFile() {},
      },
    );
    const checker = program.getTypeChecker();
    const globals = new Set([
      "Math",
      "Number",
      "undefined",
      "NaN",
      "Infinity",
      "defineProps",
      "defineEmits",
      "withDefaults",
    ]);
    if (desktop)
      for (const value of ["Object", "String", "Map", "Date"])
        globals.add(value);
    if (file === nativeAdapter) globals.add("crypto");
    function staticText(node, seen = new Set()) {
      if (!node || seen.has(node)) return null;
      seen.add(node);
      if (ts.isStringLiteralLike(node) || ts.isNumericLiteral(node))
        return node.text;
      if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node))
        return staticText(node.expression, seen);
      if (
        ts.isBinaryExpression(node) &&
        node.operatorToken.kind === ts.SyntaxKind.PlusToken
      ) {
        const left = staticText(node.left, new Set(seen)),
          right = staticText(node.right, new Set(seen));
        return left !== null && right !== null ? left + right : null;
      }
      if (ts.isIdentifier(node)) {
        const decl = checker.getSymbolAtLocation(node)?.valueDeclaration;
        if (
          decl &&
          ts.isVariableDeclaration(decl) &&
          decl.parent.flags & ts.NodeFlags.Const
        )
          return staticText(decl.initializer, seen);
      }
      return null;
    }
    const checkImport = (name) => {
      if (!allowed.includes(name) && !name.startsWith("."))
        errors.push(`${file}: unexpected dependency ${name}`);
      if (
        /(?:^|\/)(api|transport|stores?)(?:[./]|$)/.test(name) &&
        !(file === nativeAdapter && name === "@tauri-apps/api/core")
      )
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
      if (ts.isTypeNode(node)) return;
      if (ts.isImportEqualsDeclaration(node))
        errors.push(
          `${file}: dynamic module loading is not a presentation dependency`,
        );
      if (node.kind === ts.SyntaxKind.ThisKeyword)
        errors.push(`${file}: ambient this is not a presentation value`);
      if (ts.isIdentifier(node)) {
        const parent = node.parent;
        const memberName =
          (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
          ((ts.isPropertyAssignment(parent) ||
            ts.isMethodDeclaration(parent)) &&
            parent.name === node) ||
          (ts.isBindingElement(parent) && parent.propertyName === node) ||
          ts.isImportSpecifier(parent) ||
          ts.isExportSpecifier(parent);
        const symbol = ts.isShorthandPropertyAssignment(parent)
          ? checker.getShorthandAssignmentValueSymbol(parent)
          : checker.getSymbolAtLocation(node);
        const local = symbol?.declarations?.some(
          (decl) =>
            decl.getSourceFile() === ast &&
            !(ts.getCombinedModifierFlags(decl) & ts.ModifierFlags.Ambient),
        );
        if (!memberName && !local && !globals.has(node.text))
          errors.push(`${file}: ambient capability ${node.text}`);
      }
      if (
        (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier &&
        ts.isStringLiteral(node.moduleSpecifier)
      )
        checkImport(node.moduleSpecifier.text);
      if (
        file === nativeAdapter &&
        ts.isImportDeclaration(node) &&
        node.moduleSpecifier.text === "@tauri-apps/api/core"
      ) {
        const imports = node.importClause?.namedBindings;
        if (
          !imports ||
          !ts.isNamedImports(imports) ||
          imports.elements.some(
            (item) =>
              item.propertyName ||
              !["invoke", "isTauri"].includes(item.name.text),
          )
        )
          errors.push(`${file}: only direct invoke/isTauri imports allowed`);
      }
      if (
        file === nativeAdapter &&
        ts.isIdentifier(node) &&
        node.text === "invoke" &&
        !ts.isImportSpecifier(node.parent) &&
        !(ts.isCallExpression(node.parent) && node.parent.expression === node)
      )
        errors.push(`${file}: invoke cannot escape the adapter`);
      if (ts.isCallExpression(node)) {
        if (
          file === nativeAdapter &&
          node.expression.getText(ast) === "invoke" &&
          (!ts.isStringLiteral(node.arguments[0]) ||
            !fixtureCommands.includes(node.arguments[0].text))
        )
          errors.push(`${file}: only literal fixture commands allowed`);
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
        : ts.isElementAccessExpression(node)
          ? staticText(node.argumentExpression)
          : null;
      if (ts.isElementAccessExpression(node) && member === null)
        errors.push(
          `${file}: dynamic member access is not a presentation capability`,
        );
      if (
        [
          "window",
          "globalThis",
          "self",
          "document",
          "navigator",
          "location",
          "parent",
          "frames",
          "__TAURI_INTERNALS__",
          "__TAURI__",
          "fetch",
          "sendBeacon",
          "open",
          "Image",
          "WebSocket",
          "XMLHttpRequest",
          "EventSource",
          "DOMParser",
          "eval",
          "Function",
          "localStorage",
          "sessionStorage",
          "constructor",
          "__proto__",
          "ownerDocument",
          "defaultView",
          "contentWindow",
          "contentDocument",
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
        errors.push(`${file}: non-presentation member ${member}`);
      ts.forEachChild(node, visit);
    }
    if (ast.parseDiagnostics.length)
      errors.push(`${file}: invalid presentation source`);
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
  errors.push(
    ...checkSource("packages/ui/runtime.ts", read("packages/ui/runtime.ts")),
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
      .join() !== "@rss-mdm-agent/ui,@tauri-apps/api,vue" ||
    desktop.dependencies?.["@tauri-apps/api"] !== "2.11.1" ||
    Object.keys(desktop.optionalDependencies ?? {}).length ||
    Object.keys(desktop.peerDependencies ?? {}).length
  )
    errors.push(
      "desktop production dependencies must be UI, Vue and pinned Tauri core only",
    );
  const config = JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json"));
  const capabilityFiles = files(
    resolve(treeRoot, "apps/desktop/src-tauri/capabilities"),
  );
  if (capabilityFiles.length !== 1) errors.push("unexpected capabilities");
  for (const file of capabilityFiles) {
    const cap = JSON.parse(readFileSync(file, "utf8"));
    if (
      JSON.stringify(cap.windows) !== '["main"]' ||
      JSON.stringify(cap.permissions?.toSorted()) !==
        JSON.stringify(
          fixtureCommands
            .map((cmd) => `allow-${cmd.replaceAll("_", "-")}`)
            .toSorted(),
        ) ||
      cap.remote
    )
      errors.push(
        "host capability must be local main with only fixture permissions",
      );
  }
  if (
    JSON.stringify(config.app.security.capabilities) !== '["main"]' ||
    config.app.withGlobalTauri ||
    Object.keys(config.plugins ?? {}).length
  )
    errors.push("unexpected host registration");
  const policy = {
    "default-src": ["'self'"],
    "script-src": ["'self'"],
    "style-src": ["'self'", "'unsafe-inline'"],
    "img-src": ["'self'", "data:"],
    "connect-src": ["ipc:", "http://ipc.localhost"],
    "object-src": ["'none'"],
    "base-uri": ["'none'"],
    "frame-src": ["'none'"],
    "form-action": ["'none'"],
  };
  for (const mode of ["csp", "devCsp"]) {
    const expected = {
      ...policy,
      "connect-src": [
        ...policy["connect-src"],
        ...(mode === "devCsp"
          ? ["http://127.0.0.1:1420", "ws://127.0.0.1:1420"]
          : []),
      ],
    };
    const actual = new Map();
    for (const directive of String(config.app.security[mode] ?? "")
      .split(";")
      .filter((s) => s.trim())) {
      const [name, ...sources] = directive.trim().split(/\s+/);
      if (actual.has(name))
        errors.push(`${mode}: duplicate CSP directive ${name}`);
      actual.set(name, sources);
    }
    if (
      actual.size !== Object.keys(expected).length ||
      Object.entries(expected).some(
        ([name, sources]) =>
          JSON.stringify((actual.get(name) ?? []).toSorted()) !==
          JSON.stringify(sources.toSorted()),
      )
    )
      errors.push(`${mode}: CSP must match the presentation source allowlist`);
  }
  if (
    config.build.devUrl !== "http://127.0.0.1:1420" ||
    config.app.windows.length !== 1 ||
    config.app.windows[0].label !== "main" ||
    config.app.windows[0].create !== false ||
    config.app.windows[0].useHttpsScheme ||
    (config.app.windows[0].url && config.app.windows[0].url !== "index.html") ||
    config.app.security.dangerousDisableAssetCspModification ||
    config.app.security.assetProtocol?.enable
  )
    errors.push("host must create only the guarded local presentation window");
  for (const [path, expected] of [
    [
      "apps/desktop/src-tauri/Cargo.toml",
      [
        "tauri-build.workspace = true",
        "tauri.workspace = true",
        "serde.workspace = true",
        'serde_json = { workspace = true, features = ["raw_value"] }',
        "sha2.workspace = true",
        'service-catalog = { path = "../../../crates/service-catalog" }',
        'execution-contract = { path = "../../../crates/execution-contract" }',
        'execution-interaction = { path = "../../../crates/execution-interaction" }',
        'tauri = { workspace = true, features = ["test"] }',
      ],
    ],
    [
      "Cargo.toml",
      [
        'tauri = { version = "=2.11.2", features = [] }',
        'tauri-build = { version = "=2.6.2", features = [] }',
      ],
    ],
  ]) {
    let dependencySection = false;
    const dependencies = [];
    for (const line of read(path)
      .split("\n")
      .map((s) => s.replace(/#.*$/, "").trim())
      .filter(Boolean)) {
      if (line.startsWith("[")) {
        dependencySection = /dependencies/.test(line);
        if (/patch|replace/.test(line))
          errors.push(`${path}: unexpected host dependency override`);
      } else if (
        dependencySection &&
        (path !== "Cargo.toml" || /^tauri(?:-build)?\s*=/.test(line))
      ) {
        // Root workspace dependencies are shared declarations. Only the two
        // inherited desktop dependencies belong to this presentation boundary.
        dependencies.push(line);
      }
    }
    if (
      JSON.stringify(dependencies.toSorted()) !==
      JSON.stringify(expected.toSorted())
    )
      errors.push(`${path}: unexpected host dependency`);
  }
  const main = read("apps/desktop/src-tauri/src/main.rs");
  if (
    !/\.on_navigation\(navigation::allowed\)/.test(main) ||
    !/\.on_new_window\(\|_, _\| tauri::webview::NewWindowResponse::Deny\)/.test(
      main,
    )
  )
    errors.push("host must register navigation and new-window rejection");
  const ipcPath = "apps/desktop/src-tauri/src/self_service/ipc.rs";
  const ipc = read(ipcPath);
  const commands = [...ipc.matchAll(/#\[tauri::command\]\s*pub fn (\w+)/g)].map(
    (match) => match[1],
  );
  if (
    JSON.stringify(commands.toSorted()) !==
      JSON.stringify(fixtureCommands.toSorted()) ||
    (ipc.match(/\.manage\(/g) ?? []).length !== 1 ||
    (ipc.match(/\.invoke_handler\(/g) ?? []).length !== 1
  )
    errors.push(
      "fixture service must register exactly its four commands and one state owner",
    );
  const handler = ipc
    .match(/generate_handler!\[([\s\S]*?)\]/)?.[1]
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  if (
    JSON.stringify(handler?.toSorted()) !==
    JSON.stringify(fixtureCommands.toSorted())
  )
    errors.push("unexpected fixture command registration");
  const build = read("apps/desktop/src-tauri/build.rs");
  const manifest = build.match(/\.commands\(&\[([\s\S]*?)\]/)?.[1];
  const declared = [...(manifest ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
  if (
    JSON.stringify(declared.toSorted()) !==
    JSON.stringify(fixtureCommands.toSorted())
  )
    errors.push("fixture commands must be covered by the app ACL manifest");
  for (const file of files(resolve(treeRoot, "apps/desktop/src-tauri/src"))) {
    const source = readFileSync(file, "utf8");
    // Medium structural guard, not a sandbox against malicious source. Ban
    // capability modules before import aliasing can hide their call sites.
    if (
      relative(treeRoot, file)
        .replaceAll("\\", "/")
        .includes("/self_service/") &&
      (/\b(?:fs|net|process|thread|ffi|os|async_runtime)\b/.test(source) ||
        /\b(?:use|extern\s+crate)\s+(?:std|tauri)\s+(?:as\b)|\buse\s+(?:std|tauri)\s*::\s*\*/.test(
          source,
        ) ||
        /\bextern\s+"|\.\s*(?:path|shell|spawn|spawn_blocking)\s*\(/.test(
          source,
        ))
    )
      errors.push(
        `${file}: fixture service cannot access host I/O or background execution`,
      );
    if (
      /\.plugin\s*\(|Command::new|prmonitor_lib/.test(source) ||
      (relative(treeRoot, file).replaceAll("\\", "/") !== ipcPath &&
        /tauri::command|invoke_handler|\.manage\s*\(/.test(source))
    )
      errors.push(`${file}: unexpected host capability`);
  }
  return errors;
}
