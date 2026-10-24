import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname, extname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { parse, compileTemplate } from "vue/compiler-sfc";

const selfServiceCommands = [
  "device_access",
  "software_read",
  "software_prepare",
  "system_settings",
  "network_check",
  "software_submit",
  "software_status",
  "software_cancel",
  "software_history",
  "query_directory",
  "data_read",
  "query_submit",
  "query_status",
  "query_cancel",
  "self_service_snapshot",
  "self_service_execute",
  "self_service_confirm",
  "self_service_cancel",
  "self_service_policy_directory",
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
  "self_service_cancel",
  "ai_connect",
  "ai_receive",
  "ai_send",
  "ai_disconnect",
  "execution_task_details",
];
const settingsCommands = [
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
  "device_overview",
  "development_assembly",
];
const nativeAdapters = new Map([
  ["apps/desktop/src/workspace/window.ts", []],
  ["apps/desktop/src/home/native.ts", ["device_overview"]],
  ["apps/desktop/src/assembly.ts", ["development_assembly"]],
  ["apps/desktop/src/appearance.ts", ["appearance_snapshot"]],
  ["apps/desktop/src/settings/native.ts", settingsCommands],
  ["apps/desktop/src/test-users.ts", userCommands],
  ["apps/desktop/src/self-service/native.ts", selfServiceCommands],
  ["apps/desktop/src/assistant/native.ts", assistantCommands],
]);
const uiDependencies = {
  "@lucide/vue": "1.49.0",
  "markdown-it": "15.0.2",
  "reka-ui": "2.10.5",
  "@internationalized/date": "3.12.4",
};
const uiImports = {
  "apps/desktop/src/App.vue": ["@rss-mdm-agent/ai-ui-bridge"],
  "apps/desktop/src/assistant/operation-ui/NodeView.ts": [
    "@a2ui/web_core/v0_9",
    "@rss-mdm-agent/ai-contract/operations",
  ],
  "apps/desktop/src/assistant/operation-ui/OperationCard.vue": [
    "@rss-mdm-agent/ai-ui-bridge/surface",
    "@a2ui/web_core/v0_9",
    "@rss-mdm-agent/ai-contract/operations",
  ],
  "apps/desktop/src/assistant/operation-ui/host.ts": [
    "@rss-mdm-agent/ai-contract/operations",
  ],
  "apps/desktop/src/assistant/operation-ui/projection.ts": [
    "@rss-mdm-agent/ai-contract/operations",
  ],
  "packages/ui/src/index.ts": ["@lucide/vue", "reka-ui"],
  "packages/ui/src/components/AppShell.vue": ["@lucide/vue"],
  "packages/ui/src/components/MessageComposer.vue": ["@lucide/vue"],
  "packages/ui/src/internal/markdown.ts": ["markdown-it"],
};
const uiFoundations = new Set(
  [
    "UiProvider",
    "UiField",
    "UiTextInput",
    "UiNumberInput",
    "UiSearchInput",
    "UiCheckbox",
    "UiSwitch",
    "UiRadioGroup",
    "UiSlider",
    "UiSelect",
    "UiCombobox",
    "UiDateInput",
    "UiTimeInput",
    "UiDateTimeInput",
    "UiList",
    "UiTree",
    "UiTable",
    "UiTabs",
    "UiNavigation",
    "UiCollapsible",
    "UiSplitPane",
    "UiDialog",
    "UiPopover",
    "UiMenu",
    "UiTooltip",
    "UiToasts",
    "UiChart",
  ].map((name) => `packages/ui/src/components/${name}.vue`),
);
const uiLogic = new Set([
  "packages/ui/src/components/primitives.ts",
  "packages/ui/src/components/feedback.ts",
  "packages/ui/src/context.ts",
  "packages/ui/src/field.ts",
]);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
// Medium: parse UI and desktop source, including Vue template expressions.
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
  const assistant = file.startsWith("apps/desktop/src/assistant/");
  const feature = /^apps\/desktop\/src\/(assistant|self-service)\//.exec(
    file,
  )?.[1];
  const sourceDir = feature
    ? `apps/desktop/src/${feature}`
    : desktop
      ? "apps/desktop/src"
      : "packages/ui/src";
  const allowed = desktop
    ? [
        "vue",
        "@rss-mdm-agent/ui",
        "@rss-mdm-agent/ui/style.css",
        "@rss-mdm-agent/ai-contract",
        "@rss-mdm-agent/ai-contract/wire",
        "@rss-mdm-agent/ai-contract/ports",
      ]
    : ["vue"];
  allowed.push(...(uiImports[file] ?? []));
  if (
    uiFoundations.has(file) ||
    file === "packages/ui/src/components/feedback.ts"
  )
    allowed.push("reka-ui");
  if (
    [
      "packages/ui/src/components/UiDateInput.vue",
      "packages/ui/src/components/UiTimeInput.vue",
      "packages/ui/src/components/UiDateTimeInput.vue",
    ].includes(file)
  )
    allowed.push("@internationalized/date");
  if (file === "apps/desktop/src/self-service/PolicySelfService.vue")
    allowed.push("ajv");
  if (nativeAdapters.has(file)) allowed.push("@tauri-apps/api/core");
  if (file === "apps/desktop/src/workspace/window.ts")
    allowed.push("@tauri-apps/api/window");
  if (assistant)
    allowed.push(
      "@rss-mdm-agent/ai-client",
      "@rss-mdm-agent/ai-ui-bridge",
      "@rss-mdm-agent/execution-bindings",
      "@rss-mdm-agent/execution-bindings/task-details",
    );
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
        const operationRenderer =
          file ===
            "apps/desktop/src/assistant/operation-ui/OperationActivity.vue" &&
          node.tag === "component" &&
          node.props.some(
            (p) =>
              p.type === 7 &&
              p.name === "bind" &&
              p.arg?.content === "is" &&
              p.exp?.content === "renderer",
          );
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
          ].includes(node.tag.toLowerCase()) &&
          !operationRenderer
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
            ([
              "href",
              "src",
              "srcset",
              "action",
              "formaction",
              "srcdoc",
              "ping",
              "is",
            ].includes(name?.toLowerCase()) &&
              !(name === "is" && operationRenderer)) ||
            (prop.type === 7 &&
              ["bind", "on"].includes(prop.name) &&
              !name &&
              !(prop.name === "bind" && uiFoundations.has(file)))
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
    if (file === "packages/ui/src/components/MarkdownContent.ts") {
      globals.add("String");
      globals.add("Boolean");
    }
    if (uiFoundations.has(file) || uiLogic.has(file))
      for (const name of [
        "defineOptions",
        "defineExpose",
        "Object",
        "Array",
        "String",
        "Boolean",
        "Symbol",
        "Error",
      ])
        globals.add(name);
    if (file === "packages/ui/src/components/UiDialog.vue")
      for (const name of ["document", "HTMLElement"]) globals.add(name);
    if (file === "packages/ui/src/components/UiSearchInput.vue")
      for (const name of ["setTimeout", "clearTimeout"]) globals.add(name);
    if (
      [
        "packages/ui/src/components/AppShell.vue",
        "packages/ui/src/components/MessageComposer.vue",
      ].includes(file)
    )
      globals.add("ResizeObserver");
    if (file === "apps/desktop/src/assistant/clipboard.ts")
      globals.add("navigator");
    if (desktop)
      for (const value of ["Object", "Array", "String", "Map", "Date", "Error"])
        globals.add(value);
    if (
      [
        "apps/desktop/src/Workspace.vue",
        "apps/desktop/src/settings/ConnectionSettings.vue",
      ].includes(file)
    )
      globals.add("crypto");
    if (
      [
        "apps/desktop/src/test-users.ts",
        "apps/desktop/src/settings/native.ts",
        "apps/desktop/src/settings/ConnectionSettings.vue",
      ].includes(file)
    )
      globals.add("Error");
    const assistantGlobals = {
      "apps/desktop/src/workspace/page-context.ts": ["JSON"],
      "apps/desktop/src/workspace/identity.ts": ["JSON"],
      "apps/desktop/src/self-service/directory.ts": ["Set", "Symbol"],
      "apps/desktop/src/self-service/activity.ts": ["Set", "Symbol"],
      "apps/desktop/src/software/SoftwareManagement.vue": [
        "JSON",
        "document",
        "HTMLElement",
      ],
      "apps/desktop/src/device-tools/ToolsPage.vue": ["JSON"],
      "apps/desktop/src/assistant/operation-ui/NodeView.ts": [
        "Symbol",
        "document",
        "Set",
      ],
      "apps/desktop/src/App.vue": ["document", "setInterval", "clearInterval"],
      "apps/desktop/src/settings/DeviceAccess.vue": ["Promise", "crypto"],
      "apps/desktop/src/settings/MdmRegistration.vue": [
        "Promise",
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/settings/LocalService.vue": [
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/Workspace.vue": [
        "document",
        "HTMLElement",
        "ResizeObserver",
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/assistant/resource-context.ts": ["JSON"],
      "apps/desktop/src/self-service/PolicyPreparation.vue": ["JSON"],
      "apps/desktop/src/self-service/PolicySelfService.vue": [
        "defineExpose",
        "JSON",
        "crypto",
        "Set",
      ],
      "apps/desktop/src/system-settings/controller.ts": [
        "JSON",
        "Set",
        "Promise",
        "crypto",
        "Symbol",
        "setInterval",
        "clearInterval",
        "setTimeout",
      ],
      "apps/desktop/src/system-settings/SettingDetail.vue": ["JSON", "Boolean"],
      "apps/desktop/src/system-settings/SystemSettings.vue": ["JSON"],
      "apps/desktop/src/system-settings/presentation.ts": ["JSON"],
      "apps/desktop/src/device-tools/ConnectionCheck.vue": [
        "crypto",
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/settings/DiagnosticsPanel.vue": ["JSON", "Boolean"],
      "apps/desktop/src/software/controller.ts": [
        "setInterval",
        "clearInterval",
        "JSON",
        "Set",
        "Promise",
        "crypto",
        "Symbol",
      ],
      "apps/desktop/src/software/LocalSoftwareAction.vue": [
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/self-service/controller.ts": ["crypto"],
      "apps/desktop/src/appearance.ts": [
        "navigator",
        "Promise",
        "Error",
        "setTimeout",
        "clearTimeout",
      ],
      "apps/desktop/src/self-service/BackendOffers.vue": [
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/assistant/controller.ts": [
        "Set",
        "Promise",
        "AbortController",
        "setTimeout",
        "clearTimeout",
        "setInterval",
        "clearInterval",
      ],
      "apps/desktop/src/assistant/Assistant.vue": ["JSON", "ResizeObserver"],
      "apps/desktop/src/assistant/ExecutionDetails.vue": ["JSON"],
      "apps/desktop/src/assistant/operation-ui/OperationActivity.vue": [
        "AbortController",
        "JSON",
      ],
      "apps/desktop/src/assistant/operation-ui/host.ts": ["Set", "JSON"],
      "apps/desktop/src/assistant/operation-ui/projection.ts": ["JSON"],
      "apps/desktop/src/assistant/QuestionCard.vue": ["JSON"],
    };
    for (const value of assistantGlobals[file] ?? []) globals.add(value);
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
        !(nativeAdapters.has(file) && name === "@tauri-apps/api/core") &&
        !(
          file === "apps/desktop/src/workspace/window.ts" &&
          name === "@tauri-apps/api/window"
        )
      )
        errors.push(`${file}: host dependency ${name}`);
      if (
        name.startsWith(".") &&
        // The assistant shares only this data-only frozen-origin projection.
        !(
          [
            "apps/desktop/src/settings/ConnectionSettings.vue",
            "apps/desktop/src/assistant/native.ts",
            "apps/desktop/src/self-service/native.ts",
            "apps/desktop/src/self-service/PolicySelfService.vue",
          ].includes(file) && name === "../test-users"
        ) &&
        !(
          [
            "apps/desktop/src/assistant/controller.ts",
            "apps/desktop/src/assistant/resource-context.ts",
          ].includes(file) && name === "../self-service/types"
        ) &&
        !(
          (file === "apps/desktop/src/assistant/Assistant.vue" &&
            name === "../software/LocalSoftwareAction.vue") ||
          (file === "apps/desktop/src/assistant/ExecutionDetails.vue" &&
            name === "../self-service/RequestOrigin.vue") ||
          (file === "apps/desktop/src/self-service/BackendOffers.vue" &&
            name === "../assistant/resource-context") ||
          (file === "apps/desktop/src/self-service/TaskDetail.vue" &&
            name === "../assistant/ExecutionDetails.vue")
        ) &&
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
        if (
          file === "apps/desktop/src/assistant/clipboard.ts" &&
          node.text === "navigator"
        ) {
          const clipboard = node.parent,
            write = clipboard.parent,
            call = write.parent;
          if (
            !(
              ts.isPropertyAccessExpression(clipboard) &&
              clipboard.expression === node &&
              clipboard.name.text === "clipboard" &&
              ts.isPropertyAccessExpression(write) &&
              write.expression === clipboard &&
              write.name.text === "writeText" &&
              ts.isCallExpression(call) &&
              call.expression === write
            )
          )
            errors.push(
              `${file}: only direct user-initiated clipboard write allowed`,
            );
        }
        const parent = node.parent;
        const memberName =
          (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
          ((ts.isPropertyAssignment(parent) ||
            ts.isMethodDeclaration(parent)) &&
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
        nativeAdapters.has(file) &&
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
        nativeAdapters.has(file) &&
        ts.isIdentifier(node) &&
        node.text === "invoke" &&
        !ts.isImportSpecifier(node.parent) &&
        !(ts.isCallExpression(node.parent) && node.parent.expression === node)
      )
        errors.push(`${file}: invoke cannot escape the adapter`);
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
          let guardedFixture = false;
          if (
            file === "apps/desktop/src/main.ts" &&
            node.expression.kind === ts.SyntaxKind.ImportKeyword &&
            node.arguments.length === 1 &&
            ts.isStringLiteral(node.arguments[0]) &&
            node.arguments[0].text === "../../../tests/assistant/main"
          ) {
            for (let parent = node.parent; parent; parent = parent.parent) {
              if (
                ts.isIfStatement(parent) &&
                parent.expression.getText(ast) ===
                  "import.meta.env.DEV && fixtureSelected"
              )
                guardedFixture = true;
            }
          }
          const lazyPresentation = {
            "apps/desktop/src/assistant/Assistant.vue":
              "./operation-ui/QuestionSurface.vue",
            "apps/desktop/src/assistant/operation-ui/OperationActivity.vue":
              "./OperationCard.vue",
          };
          const fixedPresentation =
            node.expression.kind === ts.SyntaxKind.ImportKeyword &&
            node.arguments.length === 1 &&
            ts.isStringLiteral(node.arguments[0]) &&
            node.arguments[0].text === lazyPresentation[file];
          if (!guardedFixture && !fixedPresentation)
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
      const dataRoots = {
        "packages/ui/src/components/UiList.vue": ["_ctx.modelValue"],
        "packages/ui/src/components/UiMenu.vue": ["_ctx.items"],
        "packages/ui/src/components/UiSlider.vue": ["$event!"],
        "packages/ui/src/components/UiTree.vue": [
          "_ctx.selected",
          "selected.value",
        ],
        "packages/ui/src/components/UiTable.vue": ["row"],
        "packages/ui/src/components/UiChart.vue": [
          "series.values",
          "props.categories",
          "colors",
        ],
        "packages/ui/src/components/feedback.ts": ["marks"],
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
        "apps/desktop/src/assistant/operation-ui/NodeView.ts": [
          "c.presentation.steps",
          "nodes",
          "ids",
          "s!.draft.values",
          "s?.errors",
          "s?.secrets",
          "snapshot.current",
          "phaseLabels",
        ],
        "apps/desktop/src/self-service/PolicyPreparation.vue": [
          "action.choice.parameters",
        ],
        "apps/desktop/src/software/LocalSoftwareAction.vue": [
          "actionLabels",
          "phaseLabels",
          "_ctx.actionLabels",
          "_ctx.phaseLabels",
        ],
        "apps/desktop/src/software/SoftwareManagement.vue": [
          "kinds",
          "actionLabels",
          "blocks",
          "_ctx.kinds",
          "_ctx.actionLabels",
          "_ctx.blocks",
        ],
        "apps/desktop/src/self-service/PolicySelfService.vue": [
          "inputs.value",
          "_ctx.inputs",
          "choice.value?.parameters",
          "(parameterSchema.value as any)?.properties",
        ],
      };
      dataRoots["apps/desktop/src/device-tools/ToolsPage.vue"] = [
        "labels",
        "_ctx.labels",
      ];
      dataRoots["apps/desktop/src/device-tools/ConnectionCheck.vue"] = [
        "labels",
        "phases",
        "_ctx.labels",
        "_ctx.phases",
      ];
      dataRoots["apps/desktop/src/settings/DiagnosticsPanel.vue"] = [
        "s.scopes",
        "_ctx.s.scopes",
      ];
      dataRoots["apps/desktop/src/system-settings/controller.ts"] = [
        "families",
      ];
      dataRoots["apps/desktop/src/system-settings/SettingDetail.vue"] = [
        "proxies",
        "ports",
        "factLabels",
        "names",
        "effectLabels",
        "labels",
        "_ctx.proxies",
        "_ctx.ports",
        "_ctx.labels",
        "_ctx.effectLabels",
      ];
      const dataIndex =
        ts.isElementAccessExpression(node) &&
        (dataRoots[file] ?? []).includes(node.expression.getText(ast)) &&
        !(ts.isCallExpression(node.parent) && node.parent.expression === node);
      if (ts.isElementAccessExpression(node) && member === null && !dataIndex)
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
        ].includes(member) &&
        !(
          member === "open" &&
          uiFoundations.has(file) &&
          ts.isPropertyAccessExpression(node) &&
          ["props", "_ctx"].includes(node.expression.getText(ast)) &&
          !(ts.isCallExpression(node.parent) && node.parent.expression === node)
        )
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
    Object.keys(pkg.peerDependencies ?? {}).join() !== "vue"
  )
    errors.push(
      "UI production dependencies must be pinned presentation primitives and Markdown parser with Vue peer only",
    );
  const desktop = JSON.parse(read("apps/desktop/package.json"));
  if (
    Object.keys(desktop.dependencies ?? {})
      .sort()
      .join() !==
      "@a2ui/web_core,@rss-mdm-agent/ai-client,@rss-mdm-agent/ai-contract,@rss-mdm-agent/ai-ui-bridge,@rss-mdm-agent/execution-bindings,@rss-mdm-agent/ui,@tauri-apps/api,ajv,vue" ||
    desktop.dependencies?.["@a2ui/web_core"] !== "0.11.0" ||
    desktop.dependencies?.ajv !== "8.20.0" ||
    desktop.dependencies?.["@tauri-apps/api"] !== "2.11.1" ||
    Object.keys(desktop.optionalDependencies ?? {}).length ||
    Object.keys(desktop.peerDependencies ?? {}).length
  )
    errors.push(
      "desktop production dependencies must be UI, public AI contract/client/bridge, generated execution bindings, pinned A2UI core, pinned Ajv schema validation, Vue and pinned Tauri core only",
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
        'ai-session-contract = { path = "../../../crates/ai-session-contract" }',
        'native-process = { path = "../../../crates/native-process" }',
        'platform-private-storage = { path = "../../../crates/platform-private-storage" }',
        'platform-credentials = { path = "../../../crates/platform-credentials" }',
        'installation-security = { path = "../../../crates/installation-security" }',
        'windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_System_Threading"] }',
        'aes-gcm = { version = "=0.10.3", features = ["zeroize"] }',
        'zeroize = "=1.8.2"',
        'unicode-normalization = "=0.1.25"',
        'uuid = { version = "=1.26.0", features = ["v4"] }',
        'uuid = "=1.26.0"',
        'libc = "=0.2.189"',
        'window-vibrancy = "=0.6.0"',
        'windows = { version = "=0.61.3", features = ["UI_ViewManagement", "Win32_Graphics_Dwm", "Win32_UI_Accessibility", "Win32_UI_WindowsAndMessaging"] }',
        'windows-version = "=0.1.7"',
        'windows-sys = { version = "=0.61.2", features = ["Win32_Foundation", "Win32_Security_Credentials", "Win32_UI_Controls_Dialogs", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi", "Win32_System_Com", "Win32_UI_Shell"] }',
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
        'execution-admission = { path = "../../../crates/execution-admission" }',
        'execution-approval = { path = "../../../crates/execution-approval" }',
        'execution-capability = { path = "../../../crates/execution-capability" }',
        'execution-lifecycle = { path = "../../../crates/execution-lifecycle" }',
        'tokio = { workspace = true, features = ["process", "net", "rt-multi-thread"] }',
        "tokio-util.workspace = true",
        "futures-util.workspace = true",
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
  const ipcPath = "apps/desktop/src-tauri/src/composition/ipc.rs";
  const ipc = read(ipcPath);
  const commands =
    ipc
      .match(/commands!\s*\{([\s\S]*?)\}/)?.[1]
      .match(
        /\b(?:self_service_\w+|software_\w+|query_\w+|data_read|device_access|system_settings|network_check)(?=\s*\()/g,
      ) ?? [];
  if (
    JSON.stringify(commands.toSorted()) !==
      JSON.stringify(selfServiceCommands.toSorted()) ||
    (main.match(/app\.manage\(/g) ?? []).length !== 2 ||
    (ipc.match(/\.invoke_handler\(/g) ?? []).length !== 2
  )
    errors.push(
      "composition must register its exact command set and one state owner",
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
