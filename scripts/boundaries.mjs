import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const selfServiceCommands = [
  "task_list",
  "task_detail",
  "task_control",
  "task_control_status",
  "self_service_offers",
  "device_access",
  "software_read",
  "software_prepare",
  "system_settings",
  "network_check",
  "software_submit",
  "software_status",
  "software_history",
  "query_directory",
  "data_read",
  "query_submit",
  "query_status",
  "self_service_execute",
  "self_service_confirm",
  "self_service_policy_directory",
  "self_service_policy_resolve",
  "self_service_policy_invoke",
  "self_service_policy_confirm",
  "self_service_policy_status",
  "self_service_observe_software",
];
const assistantCommands = [
  "ai_operation_submit",
  "self_service_policy_status",
  "self_service_policy_confirm",
  "self_service_confirm",
  "ai_connect",
  "ai_receive",
  "ai_send",
  "ai_disconnect",
];
const settingsCommands = [
  "personal_vault",
  "execution_service_status",
  "agent_registration_status",
  "mdm_registration_status",
  "agent_request_registration",
  "agent_import_registration",
  "agent_cancel_registration",
  "ai_host_status",
  "ai_restart_host",
  "diagnostics_prepare",
  "diagnostics_export",
  "diagnostics_discard",
];
const userCommands = [
  "test_users",
  "select_test_user",
  "save_connection",
  "select_device",
  "account_logout",
  "account_status",
  "account_login",
  "account_recover",
  "account_organizations",
  "account_save_organization",
];
const compositionCommands = [
  ...new Set([...selfServiceCommands, ...assistantCommands]),
  ...userCommands,
  ...settingsCommands,
  "appearance_snapshot",
  "navigation",
  "device_overview",
  "development_assembly",
];
const nativeAdapters = new Map([
  ["apps/desktop/src/workspace/window.ts", []],
  ["apps/desktop/src/workspace/entry.ts", ["navigation"]],
  ["apps/desktop/src/home/native.ts", ["device_overview"]],
  ["apps/desktop/src/assembly.ts", ["development_assembly"]],
  ["apps/desktop/src/appearance.ts", ["appearance_snapshot"]],
  ["apps/desktop/src/settings/native.ts", settingsCommands],
  ["apps/desktop/src/test-users.ts", userCommands],
  ["apps/desktop/src/self-service/native.ts", selfServiceCommands],
  ["apps/desktop/src/assistant/native.ts", assistantCommands],
]);
const uiDependencies = {
  antd: "6.6.8",
  "@ant-design/icons": "6.3.4",
  "markdown-it": "15.0.2",
};
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
// Parse TSX directly: presentation cannot acquire network, HTML or native authority.
export function checkSource(file, source) {
  file = file.replaceAll("\\", "/");
  const errors = [];
  if (file.endsWith(".vue"))
    return [`${file}: retired Vue SFC is not a production source`];
  const desktop = file.startsWith("apps/desktop/src/");
  const ui = file.startsWith("packages/ui/");
  const ast = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const program = ts.createProgram(
    [file],
    { noLib: true, noResolve: true, jsx: ts.JsxEmit.ReactJSX },
    {
      ...ts.createCompilerHost({}),
      getSourceFile: (name) => (name === file ? ast : undefined),
      writeFile() {},
    },
  );
  const checker = program.getTypeChecker();
  const allowed = new Set(["react", "antd", "@ant-design/icons"]);
  if (desktop)
    for (const name of [
      "valtio",
      "valtio/vanilla",
      "valtio/vanilla/utils",
      "@rss-mdm-agent/ui",
      "@rss-mdm-agent/ui/style.css",
      "@rss-mdm-agent/ai-contract",
      "@rss-mdm-agent/ai-contract/wire",
      "@rss-mdm-agent/ai-contract/ports",
    ])
      allowed.add(name);
  if (file.startsWith("apps/desktop/src/assistant/"))
    for (const name of [
      "@rss-mdm-agent/ai-client",
      "@rss-mdm-agent/ai-ui-bridge",
      "@rss-mdm-agent/ai-ui-bridge/surface",
      "@rss-mdm-agent/ai-contract/operations",
      "@rss-mdm-agent/execution-bindings",
      "@rss-mdm-agent/execution-bindings/tasks",
    ])
      allowed.add(name);
  if (file === "packages/ui/src/UiProvider.tsx")
    allowed.add("antd/locale/zh_CN");
  if (file === "packages/ui/src/internal/markdown.ts")
    allowed.add("markdown-it");
  if (
    file === "apps/desktop/src/main.tsx" ||
    file === "apps/desktop/src/assistant/operation-ui/nodes.tsx"
  )
    allowed.add("react-dom/client");
  if (
    ["nodes.tsx", "OperationNode.tsx", "OperationCard.tsx"].some(
      (name) => file === `apps/desktop/src/assistant/operation-ui/${name}`,
    )
  )
    allowed.add("@a2ui/web_core/v0_9");
  if (nativeAdapters.has(file)) allowed.add("@tauri-apps/api/core");
  if (file === "apps/desktop/src/workspace/window.ts")
    allowed.add("@tauri-apps/api/window");
  if (file === "apps/desktop/src/workspace/entry.ts")
    allowed.add("@tauri-apps/api/event");
  if (file === "apps/desktop/src/settings/native.ts")
    allowed.add("@tauri-apps/api/app");
  if (file === "apps/desktop/src/software/controller.ts")
    allowed.add("@rss-mdm-agent/ai-ui-bridge");
  const pureGlobals = new Set([
    "Math",
    "Number",
    "String",
    "Boolean",
    "Object",
    "Array",
    "Map",
    "Set",
    "Date",
    "JSON",
    "Error",
    "Promise",
    "undefined",
    "NaN",
    "Infinity",
    "Symbol",
  ]);
  const ownerGlobals = {
    "apps/desktop/src/account/AccountPage.tsx": ["crypto"],
    "apps/desktop/src/settings/ConnectionsPage.tsx": ["crypto", "URL"],
    "apps/desktop/src/self-service/policy-preparation.ts": ["crypto"],
    "apps/desktop/src/device-tools/controller.ts": ["crypto"],
    "apps/desktop/src/shell/DesktopShell.tsx": [
      "window",
      "document",
      "HTMLElement",
    ],
    "apps/desktop/src/state/desktop.ts": [
      "document",
      "setInterval",
      "clearInterval",
    ],
    "apps/desktop/src/state/workspace.ts": ["crypto"],
    "apps/desktop/src/assistant/controller.ts": [
      "AbortController",
      "setTimeout",
      "clearTimeout",
      "setInterval",
      "clearInterval",
    ],
    "apps/desktop/src/assistant/operation-ui/host.ts": ["AbortController"],
    "apps/desktop/src/task-center/TaskDetailDrawer.tsx": [
      "document",
      "HTMLElement",
      "setInterval",
      "clearInterval",
    ],
    "apps/desktop/src/shell/PolicyDrawer.tsx": ["setInterval", "clearInterval"],
    "apps/desktop/src/software/SoftwareActions.tsx": [
      "setTimeout",
      "clearTimeout",
    ],
    "apps/desktop/src/task-center/controller.ts": [
      "setTimeout",
      "clearTimeout",
      "crypto",
      "document",
    ],
    "apps/desktop/src/software/controller.ts": [
      "setInterval",
      "clearInterval",
      "crypto",
      "document",
    ],
    "apps/desktop/src/system-settings/controller.ts": [
      "setTimeout",
      "setInterval",
      "clearInterval",
      "clearTimeout",
      "crypto",
    ],
    "apps/desktop/src/appearance.ts": [
      "navigator",
      "setTimeout",
      "clearTimeout",
    ],
    "apps/desktop/src/assistant/clipboard.ts": ["navigator"],
    "apps/desktop/src/assistant/SessionHistory.tsx": [
      "document",
      "requestAnimationFrame",
      "cancelAnimationFrame",
    ],
    "apps/desktop/src/assistant/scroll.ts": ["ResizeObserver"],
    "apps/desktop/src/assistant/operation-ui/nodes.tsx": ["WeakMap"],
    "apps/desktop/src/assistant/operation-ui/OperationCard.tsx": [
      "AbortController",
      "setInterval",
      "clearInterval",
      "document",
    ],
    "apps/desktop/src/main.tsx": ["document", "console"],
    "packages/ui/src/UiProvider.tsx": ["window"],
  };
  for (const name of ownerGlobals[file] ?? []) pureGlobals.add(name);
  const bannedMembers = new Set([
    "window",
    "globalThis",
    "self",
    "navigator",
    "location",
    "parent",
    "frames",
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
    "__TAURI_INTERNALS__",
    "__TAURI__",
  ]);
  const forbiddenTags = new Set([
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
  ]);
  const forbiddenAttributes = new Set([
    "href",
    "src",
    "srcSet",
    "action",
    "formAction",
    "srcDoc",
    "ping",
    "dangerouslySetInnerHTML",
  ]);
  const dataIndex = {
    "apps/desktop/src/device-tools/NetworkDrawer.tsx": [
      "taskPhase",
      "findings",
    ],
    "apps/desktop/src/shell/PolicyDrawer.tsx": [
      "(raw as Record<string, unknown>)",
      "values",
      "properties",
      "parameters",
      "(field.enum as unknown[])",
    ],
    "apps/desktop/src/settings/ConnectionsPage.tsx": ["providerNames"],
    "apps/desktop/src/settings/DiagnosticsPage.tsx": [
      "scopeLabels",
      "diagnostics.scopes",
      "workspace.diagnostics.state.scopes",
      "labels",
    ],
    "apps/desktop/src/software/SoftwareDetail.tsx": [
      "localActionLabels",
      "taskPhase",
    ],
    "apps/desktop/src/software/SoftwarePreparation.tsx": [
      "localActionLabels",
      "taskPhase",
    ],
    "apps/desktop/src/software/PolicyTasks.tsx": ["taskPhase"],
    "apps/desktop/src/software/SoftwareActions.tsx": [
      "taskPhase",
      "localActionLabels",
    ],
    "apps/desktop/src/software/presentation.ts": [
      "categoryTones",
      "blockReason",
    ],
    "apps/desktop/src/system-settings/SettingsEditor.tsx": ["draft", "labels"],
    "apps/desktop/src/system-settings/SystemPage.tsx": [
      "icons",
      "summaries",
      "effects",
      "labels",
    ],
    "apps/desktop/src/system-settings/presentation.ts": [
      "labels",
      "qualityLabels",
      "sourceLabels",
    ],
    "apps/desktop/src/App.tsx": ["nav.selected", "taskPhase"],
    "apps/desktop/src/home/HomePage.tsx": ["taskPhase"],
    "apps/desktop/src/shell/DesktopShell.tsx": ["layouts"],
    "apps/desktop/src/assistant/AssistantTimeline.tsx": [
      "view.commands",
      "view.messages",
      "view.surfaces",
      "view.interactions",
      "view.tools",
      "view.deliveries",
    ],
    "apps/desktop/src/assistant/Questions.tsx": [
      "view?.interactions",
      "view.interactions",
    ],
    "apps/desktop/src/assistant/operation-ui/OperationNode.tsx": [
      "state.draft.values",
      "state.errors",
      "state.secrets",
      "labels",
      "snapshot.current",
      "phaseLabels",
    ],
    "apps/desktop/src/assistant/messages.ts": [
      "view.commands",
      "view.messages",
    ],
    "apps/desktop/src/shell/GlobalSearch.tsx": ["matches"],
    "apps/desktop/src/account/LoginDialog.tsx": ["labels"],
    "apps/desktop/src/account/RegistrationPage.tsx": ["registrationLabels"],
    "apps/desktop/src/shell/navigation-model.ts": [
      "moduleIcons",
      "sections",
      "descriptions",
    ],
    "apps/desktop/src/shell/NavigationMenu.tsx": ["nav.selected"],
    "apps/desktop/src/state/navigation.ts": ["sections", "state.selected"],
    "packages/ui/src/theme.ts": ["palette"],
    "packages/ui/src/UiProvider.tsx": ["palette"],
    "packages/ui/src/components/Presentation.tsx": ["phases"],
    "apps/desktop/src/assistant/operation-ui/host.ts": [
      "event.values",
      "parameters",
      "reasons",
    ],
    "apps/desktop/src/assistant/operation-ui/projection.ts": [
      "properties",
      "action.choice.parameters",
      "snapshot.proposal",
    ],
    "apps/desktop/src/system-settings/facts.ts": ["qualityLabels"],
    "apps/desktop/src/system-settings/controller.ts": ["families"],
    "apps/desktop/src/task-center/controller.ts": [
      "state.items",
      "taskActionTitles",
      "managementTitles",
      "taskPhase",
    ],
    "apps/desktop/src/task-center/TaskCenterPage.tsx": [
      "sourceLabels",
      "counts",
    ],
    "apps/desktop/src/task-center/TaskDetailDrawer.tsx": [
      "sourceLabels",
      "actionLabels",
    ],
    "apps/desktop/src/task-center/presentation.ts": [
      "taskPhase",
      "nodeLabels",
      "explanations",
      "activityLabels",
    ],
    "apps/desktop/src/task-center/PreparationFacts.tsx": [
      "localActionLabels",
      "action.task.parameterSources",
      "action.choice.parameters",
    ],
  };
  function checkImport(name) {
    if (!name.startsWith(".") && !allowed.has(name))
      errors.push(`${file}: unexpected dependency ${name}`);
    if (name.startsWith(".")) {
      const resolved = resolve(dirname(resolve(root, file)), name).replaceAll(
        "\\",
        "/",
      );
      const scope =
        resolve(root, ui ? "packages/ui" : "apps/desktop/src").replaceAll(
          "\\",
          "/",
        ) + "/";
      if (!resolved.startsWith(scope))
        errors.push(`${file}: import escapes presentation source`);
      const feature = /^apps\/desktop\/src\/(assistant|self-service)\//.exec(
        file,
      )?.[1];
      if (
        feature &&
        !resolved.startsWith(
          resolve(root, `apps/desktop/src/${feature}`).replaceAll("\\", "/") +
            "/",
        )
      ) {
        const shared = {
          "apps/desktop/src/assistant/native.ts": [
            "../test-users",
            "../self-service/native",
          ],
          "apps/desktop/src/self-service/native.ts": ["../test-users"],
          "apps/desktop/src/assistant/controller.ts": ["../self-service/types"],
          "apps/desktop/src/assistant/AssistantPage.tsx": [
            "../state/workspace",
          ],
          "apps/desktop/src/assistant/AssistantTimeline.tsx": [
            "../state/workspace",
            "../software/presentation",
          ],
          "apps/desktop/src/assistant/Questions.tsx": [
            "../state/workspace",
            "../software/presentation",
          ],
          "apps/desktop/src/assistant/SessionHistory.tsx": [
            "../state/workspace",
            "../software/presentation",
          ],
          "apps/desktop/src/assistant/operation-ui/OperationCard.tsx": [
            "../../state/workspace",
            "../../task-center/PreparationFacts",
          ],
          "apps/desktop/src/assistant/resource-context.ts": [
            "../self-service/types",
          ],
        };
        if (!(shared[file] ?? []).includes(name))
          errors.push(`${file}: feature import escapes owner`);
      }
    }
  }
  const factories = new Set();
  const reactNamespaces = new Set();
  for (const statement of ast.statements) {
    if (
      ts.isImportDeclaration(statement) &&
      ["react", "react/jsx-runtime"].includes(statement.moduleSpecifier.text)
    ) {
      if (statement.importClause?.name)
        reactNamespaces.add(statement.importClause.name.text);
      const bindings = statement.importClause?.namedBindings;
      if (bindings && ts.isNamespaceImport(bindings))
        reactNamespaces.add(bindings.name.text);
      if (bindings && ts.isNamedImports(bindings))
        for (const entry of bindings.elements) {
          if ((entry.propertyName?.text ?? entry.name.text) === "default")
            reactNamespaces.add(entry.name.text);
          if (
            ["createElement", "cloneElement", "jsx", "jsxs"].includes(
              entry.propertyName?.text ?? entry.name.text,
            )
          )
            factories.add(entry.name.text);
        }
    }
  }
  const factoryMembers = new Set([
    "createElement",
    "cloneElement",
    "jsx",
    "jsxs",
  ]);
  function elementFactory(expression) {
    return (
      (ts.isIdentifier(expression) && factories.has(expression.text)) ||
      (ts.isPropertyAccessExpression(expression) &&
        factoryMembers.has(expression.name.text)) ||
      (ts.isElementAccessExpression(expression) &&
        ts.isStringLiteralLike(expression.argumentExpression) &&
        factoryMembers.has(expression.argumentExpression.text))
    );
  }
  function isBoundaryThis(node) {
    if (
      file !== "apps/desktop/src/assistant/operation-ui/nodes.tsx" ||
      node.kind !== ts.SyntaxKind.ThisKeyword
    )
      return false;
    let owner = node.parent;
    while (owner && !ts.isClassDeclaration(owner)) owner = owner.parent;
    return (
      owner?.name?.text === "CardBoundary" &&
      owner.heritageClauses?.some(
        (clause) =>
          clause.token === ts.SyntaxKind.ExtendsKeyword &&
          clause.types.some(
            (type) => type.expression.getText(ast) === "Component",
          ),
      ) &&
      ast.statements.some(
        (statement) =>
          ts.isImportDeclaration(statement) &&
          statement.moduleSpecifier.text === "react" &&
          statement.importClause?.namedBindings &&
          ts.isNamedImports(statement.importClause.namedBindings) &&
          statement.importClause.namedBindings.elements.some(
            (binding) =>
              binding.name.text === "Component" && !binding.propertyName,
          ),
      )
    );
  }
  function visit(node) {
    if (ts.isTypeNode(node)) return;
    if (
      ts.isImportEqualsDeclaration(node) ||
      (node.kind === ts.SyntaxKind.ThisKeyword && !isBoundaryThis(node))
    )
      errors.push(`${file}: ambient module/this capability`);
    if (
      (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
      node.moduleSpecifier &&
      ts.isStringLiteralLike(node.moduleSpecifier)
    )
      checkImport(node.moduleSpecifier.text);
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      if (forbiddenTags.has(node.tagName.getText(ast)))
        errors.push(`${file}: non-presentation element`);
      if (node.tagName.getText(ast) === "form")
        errors.push(
          `${file}: use controlled Ant Design Form without native submission`,
        );
    }
    if (
      ts.isJsxAttribute(node) &&
      forbiddenAttributes.has(node.name.getText(ast))
    )
      errors.push(`${file}: outbound or executable JSX attribute`);
    if (
      elementFactory(node) &&
      !ts.isImportSpecifier(node.parent) &&
      !(ts.isCallExpression(node.parent) && node.parent.expression === node)
    )
      errors.push(`${file}: element factory cannot escape inspection`);
    if (ts.isJsxSpreadAttribute(node))
      errors.push(`${file}: unchecked JSX attributes`);
    if (ts.isBindingElement(node)) {
      const name = node.propertyName ?? node.name;
      if (
        (ts.isIdentifier(name) || ts.isStringLiteralLike(name)) &&
        factoryMembers.has(name.text)
      )
        errors.push(`${file}: destructured element factory escapes inspection`);
    }
    if (
      ts.isExportDeclaration(node) &&
      node.moduleSpecifier &&
      ["react", "react/jsx-runtime"].includes(node.moduleSpecifier.text) &&
      !node.isTypeOnly &&
      !(
        node.exportClause &&
        ts.isNamedExports(node.exportClause) &&
        node.exportClause.elements.every((entry) => entry.isTypeOnly)
      )
    )
      errors.push(`${file}: React value exports escape capability inspection`);
    if (ts.isIdentifier(node)) {
      const parent = node.parent;
      if (
        reactNamespaces.has(node.text) &&
        !ts.isImportClause(parent) &&
        !ts.isImportSpecifier(parent) &&
        !ts.isNamespaceImport(parent) &&
        !(
          ts.isPropertyAccessExpression(parent) && parent.expression === node
        ) &&
        !(ts.isElementAccessExpression(parent) && parent.expression === node)
      )
        errors.push(`${file}: React namespace cannot escape member inspection`);
      const jsxName =
        (ts.isJsxAttribute(parent) && parent.name === node) ||
        ((ts.isJsxOpeningElement(parent) ||
          ts.isJsxClosingElement(parent) ||
          ts.isJsxSelfClosingElement(parent)) &&
          parent.tagName === node &&
          /^[a-z]/.test(node.text));
      const member =
        (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
        ((ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent)) &&
          parent.name === node) ||
        (ts.isBindingElement(parent) && parent.propertyName === node) ||
        ts.isImportSpecifier(parent) ||
        ts.isExportSpecifier(parent) ||
        ts.isMetaProperty(parent);
      const symbol = ts.isShorthandPropertyAssignment(parent)
        ? checker.getShorthandAssignmentValueSymbol(parent)
        : checker.getSymbolAtLocation(node);
      const local = symbol?.declarations?.some(
        (decl) =>
          decl.getSourceFile() === ast &&
          !(ts.getCombinedModifierFlags(decl) & ts.ModifierFlags.Ambient),
      );
      if (!jsxName && !member && !local && !pureGlobals.has(node.text))
        errors.push(`${file}: ambient capability ${node.text}`);
      if (
        node.text === "window" &&
        file === "packages/ui/src/UiProvider.tsx" &&
        !(
          ts.isPropertyAccessExpression(parent) &&
          parent.name.text === "matchMedia" &&
          ts.isCallExpression(parent.parent) &&
          parent.parent.expression === parent
        )
      )
        errors.push(`${file}: only system appearance reads allowed`);
      if (
        node.text === "navigator" &&
        file === "apps/desktop/src/assistant/clipboard.ts"
      ) {
        const property = parent,
          call = parent.parent;
        if (
          !(
            ts.isPropertyAccessExpression(property) &&
            property.name.text === "clipboard" &&
            ts.isPropertyAccessExpression(call) &&
            call.name.text === "writeText" &&
            ts.isCallExpression(call.parent)
          )
        )
          errors.push(`${file}: only direct clipboard write allowed`);
      }
      if (
        node.text === "invoke" &&
        nativeAdapters.has(file) &&
        !ts.isImportSpecifier(parent) &&
        !(ts.isCallExpression(parent) && parent.expression === node)
      )
        errors.push(`${file}: invoke cannot escape adapter`);
    }
    if (
      ts.isImportDeclaration(node) &&
      node.moduleSpecifier.text === "@tauri-apps/api/core"
    ) {
      const bindings = node.importClause?.namedBindings;
      if (
        !nativeAdapters.has(file) ||
        !bindings ||
        !ts.isNamedImports(bindings) ||
        bindings.elements.some(
          (item) =>
            item.propertyName ||
            !["invoke", "isTauri"].includes(item.name.text),
        )
      )
        errors.push(`${file}: native capability owner`);
    }
    if (ts.isCallExpression(node) && elementFactory(node.expression)) {
      const tag = node.arguments[0],
        props = node.arguments[1];
      if (
        !tag ||
        !ts.isStringLiteralLike(tag) ||
        (ts.isStringLiteralLike(tag) &&
          (forbiddenTags.has(tag.text) || tag.text === "form"))
      )
        errors.push(`${file}: non-presentation factory element`);
      if (props && props.kind !== ts.SyntaxKind.NullKeyword) {
        if (!ts.isObjectLiteralExpression(props))
          errors.push(`${file}: unchecked factory props`);
        else
          for (const property of props.properties) {
            const name =
              property.name &&
              (ts.isIdentifier(property.name) ||
                ts.isStringLiteralLike(property.name))
                ? property.name.text
                : null;
            if (name === null || forbiddenAttributes.has(name))
              errors.push(`${file}: outbound or executable factory property`);
          }
      }
    }
    if (ts.isCallExpression(node)) {
      if (
        nativeAdapters.has(file) &&
        node.expression.getText(ast) === "invoke" &&
        (!ts.isStringLiteral(node.arguments[0]) ||
          !nativeAdapters.get(file).includes(node.arguments[0].text))
      )
        errors.push(
          `${file}: only declared literal composition commands allowed`,
        );
      if (
        node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        node.expression.getText(ast) === "require"
      ) {
        let fixture = false;
        if (
          file === "apps/desktop/src/main.tsx" &&
          node.arguments.length === 1 &&
          ts.isStringLiteral(node.arguments[0]) &&
          node.arguments[0].text === "../../../tests/assistant/main"
        ) {
          for (let parent = node.parent; parent; parent = parent.parent)
            if (
              ts.isIfStatement(parent) &&
              parent.expression.getText(ast) ===
                "import.meta.env.DEV && fixtureSelected"
            )
              fixture = true;
        }
        if (!fixture) errors.push(`${file}: dynamic module loading`);
      }
    }
    if (
      ts.isPropertyAccessExpression(node) ||
      ts.isElementAccessExpression(node)
    ) {
      const literal = ts.isPropertyAccessExpression(node)
        ? node.name.text
        : ts.isStringLiteralLike(node.argumentExpression) ||
            ts.isNumericLiteral(node.argumentExpression)
          ? node.argumentExpression.text
          : null;
      if (
        literal === null &&
        !(dataIndex[file] ?? []).includes(node.expression.getText(ast))
      )
        errors.push(`${file}: dynamic member capability`);
      // TaskNode.parent is an owner-issued string identity, not a DOM browsing context.
      if (
        bannedMembers.has(literal) &&
        !(
          file === "apps/desktop/src/task-center/TaskDetailDrawer.tsx" &&
          node.getText(ast) === "node.parent"
        )
      )
        errors.push(`${file}: non-presentation member ${literal}`);
    }
    ts.forEachChild(node, visit);
  }
  if (ast.parseDiagnostics.length)
    errors.push(`${file}: invalid presentation source`);
  if (
    /\b(prNumber|pr_number|PullRequestView|usePrStore|useReviewStore)\b/.test(
      source,
    )
  )
    errors.push(`${file}: business coupling`);
  visit(ast);
  return errors;
}
export function checkTree(treeRoot = root, executable) {
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
    JSON.stringify(Object.entries(pkg.dependencies ?? {}).sort()) !==
      JSON.stringify(Object.entries(uiDependencies).sort()) ||
    Object.keys(pkg.optionalDependencies ?? {}).length ||
    Object.keys(pkg.peerDependencies ?? {})
      .sort()
      .join() !== "react,react-dom"
  )
    errors.push(
      "UI production dependencies must be pinned presentation primitives and Markdown parser with React peers only",
    );
  const desktop = JSON.parse(read("apps/desktop/package.json"));
  if (
    Object.keys(desktop.dependencies ?? {})
      .sort()
      .join() !==
      "@a2ui/web_core,@ant-design/icons,@rss-mdm-agent/ai-client,@rss-mdm-agent/ai-contract,@rss-mdm-agent/ai-ui-bridge,@rss-mdm-agent/execution-bindings,@rss-mdm-agent/ui,@tauri-apps/api,antd,react,react-dom,valtio" ||
    desktop.dependencies?.valtio !== "2.3.2" ||
    desktop.dependencies?.["@a2ui/web_core"] !== "0.11.0" ||
    desktop.dependencies?.["@tauri-apps/api"] !== "2.11.1" ||
    Object.keys(desktop.optionalDependencies ?? {}).length ||
    Object.keys(desktop.peerDependencies ?? {}).length
  )
    errors.push(
      "desktop production dependencies must be UI, public AI contract/client/bridge, generated execution bindings, pinned A2UI core, React, Ant Design and pinned Tauri core only",
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
          [
            ...compositionCommands.map(
              (cmd) => `allow-${cmd.replaceAll("_", "-")}`,
            ),
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "core:window:allow-start-dragging",
            "core:window:allow-internal-toggle-maximize",
            "core:window:allow-minimize",
            "core:window:allow-toggle-maximize",
            "core:window:allow-close",
          ].toSorted(),
        ) ||
      cap.remote
    )
      errors.push(
        "host capability must be local main with only declared composition permissions",
      );
  }
  if (
    JSON.stringify(config.app.security.capabilities) !== '["main"]' ||
    config.app.withGlobalTauri ||
    JSON.stringify(config.plugins) !==
      JSON.stringify({ "deep-link": { desktop: { schemes: ["rssmdm"] } } })
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
        'ai-session-contract = { path = "../../../crates/ai-session-contract" }',
        'native-process = { path = "../../../crates/native-process" }',
        'platform-private-storage = { path = "../../../crates/platform-private-storage" }',
        'platform-credentials = { path = "../../../crates/platform-credentials" }',
        'passkey-vault = { path = "../../../crates/passkey-vault" }',
        'windows-passkey-provider = { path = "../../../crates/windows-passkey-provider" }',
        'macos-passkey-provider = { path = "../../../crates/macos-passkey-provider" }',
        'installation-security = { path = "../../../crates/installation-security" }',
        'windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_System_Threading"] }',
        'aes-gcm = { version = "=0.10.3", features = ["zeroize"] }',
        'zeroize = "=1.8.2"',
        'unicode-normalization = "=0.1.25"',
        'uuid = { version = "=1.26.0", features = ["v4"] }',
        'uuid = "=1.26.0"',
        'libc = "=0.2.189"',
        'window-vibrancy = "=0.6.0"',
        'windows = { version = "=0.61.3", features = ["Security_Credentials_UI", "Win32_System_WinRT", "UI_ViewManagement", "Win32_Graphics_Dwm", "Win32_UI_Accessibility", "Win32_UI_WindowsAndMessaging"] }',
        'windows-version = "=0.1.7"',
        'windows-future = "=0.2.1"',
        'windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_Security_Credentials", "Win32_UI_Controls_Dialogs", "Win32_UI_Controls", "Win32_UI_Input_KeyboardAndMouse", "Win32_System_RemoteDesktop", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi", "Win32_System_Com", "Win32_UI_Shell"] }',
        'objc2 = "=0.6.4"',
        'objc2-app-kit = { version = "=0.3.2", default-features = false, features = ["std", "NSAlert", "NSButton", "NSControl", "NSSecureTextField", "NSTextField", "NSView", "NSResponder", "NSWindow", "NSApplication", "NSSavePanel", "NSOpenPanel", "NSPanel", "NSWorkspace", "NSAccessibility"] }',
        'objc2-foundation = { version = "=0.3.2", default-features = false, features = ["std", "NSString", "NSGeometry", "NSURL"] }',
        'execution-app = { path = "../../../crates/execution-app" }',
        'execution-app = { path = "../../../crates/execution-app", features = ["test-support"] }',
        'execution-ipc = { path = "../../../crates/execution-ipc" }',
        'execution-interaction = { path = "../../../crates/execution-interaction" }',
        "rusqlite.workspace = true",
        'execution-sqlite = { path = "../../../crates/execution-sqlite", features = ["test-support"] }',
        'execution-mcp = { path = "../../../crates/execution-mcp" }',
        'execution-lifecycle = { path = "../../../crates/execution-lifecycle" }',
        'tokio = { workspace = true, features = ["process", "net", "rt-multi-thread"] }',
        "tokio-util.workspace = true",
        "futures-util.workspace = true",
        'tauri-plugin-deep-link = "=2.4.10"',
        'tauri-plugin-wdio-webdriver = { path = "../../../vendor/tauri-plugin-wdio-webdriver", version = "=1.4.0", optional = true }',
        "tauri-build.workspace = true",
        // Build-only compiler for the fixed LocalAuthentication presence bridge.
        'cc = "=1.4.5"',
        // Build-time validation reuses the same serialization and URL parser as native connections.
        "serde.workspace = true",
        "url.workspace = true",
        'tauri = { workspace = true, features = ["macos-private-api", "process-relaunch-dangerous-allow-symlink-macos"] }',
        "serde.workspace = true",
        'serde_json = { workspace = true, features = ["raw_value"] }',
        "serde_json.workspace = true",
        "sha2.workspace = true",
        'reqwest = { version = "=0.13.5", default-features = false, features = ["json", "native-tls"] }',
        "url.workspace = true",
        'constant_time_eq = "=0.4.2"',
        "schemars.workspace = true",
        'syn = { version = "=2.0.119", features = ["full", "visit"] }',
        'proc-macro2 = "=1.0.107"',
        'execution-contract = { path = "../../../crates/execution-contract" }',
        "rss-mdm-agent-wire.workspace = true",
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
  const nativeManifest = read("apps/desktop/src-tauri/Cargo.toml");
  if (
    !/^default = \["custom-protocol"\]$/m.test(nativeManifest) ||
    !/^native-e2e = \["dep:tauri-plugin-wdio-webdriver"\]$/m.test(
      nativeManifest,
    ) ||
    !/#\[cfg\(feature = "native-e2e"\)\]\s*mod native_e2e;/.test(main) ||
    !/#\[cfg\(feature = "native-e2e"\)\]\s*let builder = native_e2e::configure\(builder, _data_root\.as_deref\(\)\)\?;/.test(
      main,
    ) ||
    !/#\[cfg\(all\(feature = "native-e2e", not\(debug_assertions\)\)\)\]\s*compile_error!/.test(
      main,
    )
  )
    errors.push(
      "native WebDriver must be opt-in, isolated and excluded from release",
    );
  if (
    !/\.on_navigation\(navigation::allowed\)/.test(main) ||
    !/\.on_new_window\(\|_, _\| tauri::webview::NewWindowResponse::Deny\)/.test(
      main,
    )
  )
    errors.push("host must register navigation and new-window rejection");
  if (!/vault\.bind\(&created\)/.test(main))
    errors.push(
      "main window creation must bind the personal Vault native lifecycle",
    );
  const ipcPath = "apps/desktop/src-tauri/src/composition/ipc.rs";
  const ipc = read(ipcPath);
  const commands =
    ipc
      .match(/commands!\s*\{([\s\S]*?)\}/)?.[1]
      .match(
        /\b(?:self_service_\w+|task_\w+|software_\w+|query_\w+|data_read|device_access|system_settings|network_check)(?=\s*\()/g,
      ) ?? [];
  if (
    JSON.stringify(commands.toSorted()) !==
      JSON.stringify(selfServiceCommands.toSorted()) ||
    (main.match(/app\.manage\(/g) ?? []).length !== 5 ||
    (ipc.match(/\.invoke_handler\(/g) ?? []).length !== 2
  )
    errors.push(
      "composition must register its exact command set and fixed native state owners",
    );
  const registered =
    ipc
      .match(/generate_handler!\s*\[([\s\S]*?)\]/)?.[1]
      .match(/\b[a-z_][a-z0-9_]*\b/g) ?? [];
  if (
    JSON.stringify(registered.toSorted()) !==
    JSON.stringify(compositionCommands.toSorted())
  )
    errors.push("exactly the declared composition commands must be registered");
  const build = read("apps/desktop/src-tauri/build.rs");
  const manifest = build.match(/\.commands\(&\[([\s\S]*?)\]/)?.[1];
  const declared = [...(manifest ?? "").matchAll(/"(\w+)"/g)].map((m) => m[1]);
  if (
    JSON.stringify(declared.toSorted()) !==
    JSON.stringify(compositionCommands.toSorted())
  )
    errors.push("composition commands must be covered by the app ACL manifest");
  const rustSources = Object.fromEntries(
    files(resolve(treeRoot, "apps/desktop/src-tauri/src"))
      .filter((file) => file.endsWith(".rs"))
      .map((file) => [
        relative(treeRoot, file).replaceAll("\\", "/"),
        readFileSync(file, "utf8"),
      ]),
  );
  errors.push(...checkRustSources(rustSources, executable));
  return errors;
}

export function checkRustSources(sources, executable) {
  if (!executable) throw Error("prepared Rust boundary executable required");
  return JSON.parse(
    execFileSync(executable, [], {
      cwd: root,
      input: JSON.stringify(sources),
      encoding: "utf8",
    }),
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const { withPreparation } = await import("./preparation.mjs");
  const { runCommand } = await import("./ci-plan.mjs");
  process.exitCode = await withPreparation(
    ["boundaries"],
    async (env, signal, nested) => {
      const result = await runCommand(
        process.execPath,
        ["--test", "scripts/boundaries.test.mjs"],
        {
          cwd: root,
          env,
          signal,
          stdio: "inherit",
          ...(nested ? { detached: false } : {}),
        },
      );
      return result.status ?? 1;
    },
  );
}
