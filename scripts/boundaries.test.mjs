import {
  cpSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  checkTree as inspectTree,
  checkSource,
  checkRustSources as inspectRust,
} from "./boundaries.mjs";
import { preparedArtifact } from "./preparation.mjs";
const rustGuard = preparedArtifact("boundaries");
const checkTree = (treeRoot) => inspectTree(treeRoot, rustGuard);
const checkRustSources = (sources) => inspectRust(sources, rustGuard);

// ref: Cargo 0.90.0 src/cargo/core/resolver/features.rs (FeatureOpts/HasDevUnits).
// Inspect the resolved production graph, without the dev edges used by test/example targets.
test("production service and desktop resolve without test-support", () => {
  const graph = execFileSync(
    "cargo",
    [
      "tree",
      "--locked",
      "-p",
      "agent-service",
      "-p",
      "rss-mdm-desktop",
      "--edges",
      "normal,build",
      "--prefix",
      "none",
      "--format",
      "{p}|{f}",
    ],
    { cwd: new URL("../", import.meta.url), encoding: "utf8" },
  );
  assert.ok(graph.includes("agent-service v"));
  assert.ok(graph.includes("rss-mdm-desktop v"));
  for (const line of graph.trim().split("\n").filter(Boolean)) {
    const [identity, features] = line.split("|");
    assert.ok(features !== undefined, `missing resolved features: ${line}`);
    assert.ok(
      !features.split(/[,\s]+/).includes("test-support"),
      `${identity} enables test-support in the production dependency graph`,
    );
  }
});

test("Rust guard distinguishes syntax from harmless words and literal contents", () => {
  const file = "apps/desktop/src-tauri/src/self_service/example.rs";
  assert.deepEqual(
    checkRustSources({
      [file]: `
    // No fs, net, process, thread, ffi, os or async_runtime capabilities here.
    /* nested /* std::fs::read("x") */ comment */
    fn process() { let net = "std::process::Command::new";
      let fs = r###"extern \\\"C\\\"; app.shell(); tauri::async_runtime::spawn"###;
      let _ = (net, fs, b"std::fs", '\\'');
    }
  `,
    }),
    [],
  );
  for (const code of [
    'use std::{net::{TcpStream as Connection}}; fn run() { Connection::connect("x"); }',
    'use std::{self as host}; fn run() { host::fs::read("x"); }',
    'macro_rules! run { () => { std::fs::read("x") }; }',
    'macro_rules! run { () => { use std as host; host::fs::read("x"); }; }',
    "macro_rules! run { () => { app.shell(); }; }",
    'fn run() { /* between tokens */ std /* comment */ :: fs::read("x"); }',
    "fn unfinished(",
  ])
    assert.ok(checkRustSources({ [file]: code }).length, code);
});

test("UI and desktop satisfy the dependency and permission boundaries", () => {
  assert.deepEqual(checkTree(), []);
});
test("Rust guard excludes explicit test modules while guarding production IPC assembly", () => {
  const file = "apps/desktop/src-tauri/src/composition/ipc.rs";
  const mock = "mod tests { fn check() { builder.manage(state); } }";
  assert.deepEqual(checkRustSources({ [file]: `#[cfg(test)] ${mock}` }), []);
  for (const source of [mock, `#[cfg(feature = "test")] ${mock}`])
    assert.ok(checkRustSources({ [file]: source }).length);
});
test("only the native acceptance owner can register the pinned WebDriver plugin", () => {
  const file = "apps/desktop/src-tauri/src/native_e2e.rs";
  const source =
    "fn run() { builder.plugin(tauri_plugin_wdio_webdriver::init(socket, capability)); }";
  assert.deepEqual(checkRustSources({ [file]: source }), []);
  for (const [path, code] of [
    ["apps/desktop/src-tauri/src/main.rs", source],
    [file, "fn run() { builder.plugin(arbitrary()); }"],
    [
      file,
      "fn run() { builder.plugin(tauri_plugin_wdio_webdriver::init_with_port(port)); }",
    ],
    [file, "fn run() { builder.invoke_handler(arbitrary()); }"],
  ])
    assert.ok(checkRustSources({ [path]: code }).length);
});

test("tree scan covers desktop files and both production manifests", () => {
  const dir = mkdtempSync(join(tmpdir(), "ui-boundaries-"));
  try {
    for (const path of [
      "packages/ui/src",
      "packages/ui/package.json",
      "packages/ui/runtime.ts",
      "Cargo.toml",
      "apps/desktop/src",
      "apps/desktop/package.json",
      "apps/desktop/src-tauri",
    ])
      cpSync(new URL(`../${path}`, import.meta.url), join(dir, path), {
        recursive: true,
      });
    writeFileSync(
      join(dir, "apps/desktop/src/regression.ts"),
      `fetch('https://example.com')`,
    );
    assert.ok(checkTree(dir).some((e) => e.includes("regression.ts")));
    for (const path of [
      "apps/desktop/package.json",
      "packages/ui/package.json",
    ]) {
      const file = join(dir, path);
      const pkg = JSON.parse(readFileSync(file, "utf8"));
      pkg.optionalDependencies = { "@tauri-apps/api": "2.0.0" };
      writeFileSync(file, JSON.stringify(pkg));
    }
    const errors = checkTree(dir);
    assert.ok(
      errors.includes(
        "UI production dependencies must be pinned presentation primitives and Markdown parser with React peers only",
      ),
    );
    assert.ok(
      errors.includes(
        "desktop production dependencies must be UI, public AI contract/client/bridge, generated execution bindings, pinned A2UI core, React, Ant Design and pinned Tauri core only",
      ),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("each host boundary mutation independently fails the tree scan", () => {
  const dir = mkdtempSync(join(tmpdir(), "ui-host-boundaries-"));
  const mutations = [
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) => s.replace("vault.bind(&created);", ""),
    ],
    [
      "apps/desktop/src-tauri/Cargo.toml",
      (s) =>
        s.replace(
          'default = ["custom-protocol"]',
          'default = ["custom-protocol", "native-e2e"]',
        ),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) =>
        s.replace(
          '#[cfg(all(feature = "native-e2e", not(debug_assertions)))]',
          "#[cfg(any())]",
        ),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) => s.replaceAll('#[cfg(feature = "native-e2e")]', ""),
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => {
        const c = JSON.parse(s);
        c.app.security.csp = "default-src *";
        return JSON.stringify(c);
      },
    ],
    ...[
      "object-src 'none'",
      "frame-src 'none'",
      "base-uri 'none'",
      "form-action 'none'",
    ].map((d) => [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => s.replaceAll(d, ""),
    ]),
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) =>
        s.replaceAll(
          "connect-src ipc:",
          "connect-src https://example.com ipc:",
        ),
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => {
        const c = JSON.parse(s);
        c.app.security.devCsp = "default-src *";
        return JSON.stringify(c);
      },
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => s.replace('"withGlobalTauri": false', '"withGlobalTauri": true'),
    ],
    [
      "apps/desktop/src-tauri/capabilities/main.json",
      (s) => {
        const c = JSON.parse(s);
        c.permissions.push("core:default");
        return JSON.stringify(c);
      },
    ],
    [
      "apps/desktop/src-tauri/capabilities/main.json",
      (s) => {
        const c = JSON.parse(s);
        c.windows = ["*"];
        return JSON.stringify(c);
      },
    ],
    [
      "apps/desktop/src-tauri/Cargo.toml",
      (s) => s.replace("[dependencies]", '[dependencies]\nureq = "3"'),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) =>
        s.replace(
          "tauri::Builder::default()",
          "tauri::Builder::default().invoke_handler(handler)",
        ),
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => s.replace('"create": false', '"create": true'),
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) => s.replace("ws://127.0.0.1:1420", "ws://*"),
    ],
    [
      "apps/desktop/src-tauri/tauri.conf.json",
      (s) =>
        s.replace("default-src 'self'", "default-src 'self'; default-src *"),
    ],
    [
      "apps/desktop/src-tauri/capabilities/main.json",
      (s) =>
        s.replace(
          '"identifier": "main"',
          '"identifier": "main", "remote": {"urls": ["https://*"]}',
        ),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) => s.replace(".on_navigation(navigation::allowed)", ""),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) =>
        s.replace(
          ".on_navigation(navigation::allowed)",
          ".on_navigation(|_| true)",
        ),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) => s.replace("NewWindowResponse::Deny", "NewWindowResponse::Allow"),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) =>
        s.replace(
          "tauri::Builder::default()",
          "tauri::Builder::default().plugin(plugin)",
        ),
    ],
    [
      "apps/desktop/src-tauri/src/main.rs",
      (s) =>
        s.replace(
          "tauri::Builder::default()",
          "tauri::Builder::default().manage(state)",
        ),
    ],
    [
      "Cargo.toml",
      (s) => s.replace("features = []", 'features = ["devtools"]'),
    ],
    ...[
      'std::net::TcpStream::connect("127.0.0.1:9");',
      'std::fs::read("local-file");',
      "std::thread::spawn(|| {});",
      'use std::{process::Command as Launcher}; Launcher::new("test");',
      'use std::{net as network}; network::TcpStream::connect("127.0.0.1:9");',
      'use std as host; host::fs::read("local-file");',
      'use std::*; fs::read("local-file");',
      "tauri::async_runtime::spawn(async {});",
      "app.path().app_data_dir();",
      'extern "C" { fn system(); }',
    ].map((capability) => [
      "apps/desktop/src-tauri/src/self_service/model.rs",
      (source) => source + `\nfn forbidden() { ${capability} }\n`,
    ]),
  ];
  try {
    for (const path of [
      "packages/ui/src",
      "packages/ui/package.json",
      "packages/ui/runtime.ts",
      "apps/desktop/src",
      "apps/desktop/package.json",
      "apps/desktop/src-tauri",
      "Cargo.toml",
    ])
      cpSync(new URL(`../${path}`, import.meta.url), join(dir, path), {
        recursive: true,
      });
    for (const [path, mutate] of mutations) {
      assert.deepEqual(
        checkTree(dir),
        [],
        "unchanged fixture is valid before each mutation",
      );
      const file = join(dir, path);
      const original = readFileSync(file, "utf8");
      const changed = mutate(original);
      assert.notEqual(changed, original, path);
      writeFileSync(file, changed);
      try {
        assert.ok(checkTree(dir).length, `${path}: ${changed}`);
      } finally {
        writeFileSync(file, original);
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("root workspace may declare independent core dependencies without changing desktop authority", () => {
  const dir = mkdtempSync(join(tmpdir(), "ui-workspace-boundaries-"));
  try {
    for (const path of [
      "packages/ui/src",
      "packages/ui/package.json",
      "packages/ui/runtime.ts",
      "Cargo.toml",
      "apps/desktop/src",
      "apps/desktop/package.json",
      "apps/desktop/src-tauri",
    ])
      cpSync(new URL(`../${path}`, import.meta.url), join(dir, path), {
        recursive: true,
      });
    const file = join(dir, "Cargo.toml");
    writeFileSync(
      file,
      readFileSync(file, "utf8") + '\nindependent-core = "1"\n',
    );
    assert.deepEqual(checkTree(dir), []);
    writeFileSync(
      file,
      readFileSync(file, "utf8").replace(
        "features = []",
        'features = ["devtools"]',
      ),
    );
    assert.ok(
      checkTree(dir).some((e) => e.includes("unexpected host dependency")),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("only the narrow native adapter can call literal fixture commands", () => {
  const file = "apps/desktop/src/self-service/native.ts";
  for (const source of [
    `import { invoke } from '@tauri-apps/api/core'; invoke('run_shell')`,
    `import { invoke } from '@tauri-apps/api/core'; const name = 'task_list'; invoke(name)`,
    `import { invoke as call } from '@tauri-apps/api/core'; call('task_list')`,
    `import { invoke } from '@tauri-apps/api/core'; export const call = invoke`,
    `import * as native from '@tauri-apps/api/core'; native.invoke('run_shell')`,
  ])
    assert.ok(checkSource(file, source).length, source);
  assert.deepEqual(
    checkSource(
      file,
      `import { invoke } from '@tauri-apps/api/core'; invoke('task_list')`,
    ),
    [],
  );
});

test("fixture presentation cannot import the desktop composition owner", () => {
  for (const source of [
    "fn preview() { crate::composition::origin::human(); }",
    "use crate::{composition::origin as trusted}; fn preview() { trusted::human(); }",
    "use super::super::composition;",
  ])
    assert.ok(
      checkRustSources({
        "apps/desktop/src-tauri/src/self_service/example.rs": source,
      }).length,
    );
});

test("test-user adapter owns only literal user and native credential commands", () => {
  const file = "apps/desktop/src/test-users.ts";
  assert.deepEqual(
    checkSource(
      file,
      'import { invoke } from "@tauri-apps/api/core"; void invoke("test_users"); void invoke("save_connection", {});',
    ),
    [],
  );
  for (const source of [
    'import { invoke } from "@tauri-apps/api/core"; void invoke("ai_send", {});',
    'import { invoke } from "@tauri-apps/api/core"; const command = "test_users"; void invoke(command);',
    'import { invoke } from "@tauri-apps/api/core"; const escaped = invoke;',
    'import x from "@rss-mdm-agent/ai-contract/session";',
    'fetch("https://example.invalid");',
  ])
    assert.ok(checkSource(file, source).length, source);
});

test("appearance adapter exposes only its literal read-only command", () => {
  const file = "apps/desktop/src/appearance.ts";
  assert.deepEqual(
    checkSource(
      file,
      'import { invoke } from "@tauri-apps/api/core"; invoke("appearance_snapshot");',
    ),
    [],
  );
  assert.ok(
    checkSource(
      file,
      'import { invoke } from "@tauri-apps/api/core"; invoke("self_service_execute", {});',
    ).length,
  );
});

test("registration adapters admit only fixed native actions and scoped status polling", () => {
  const native = "apps/desktop/src/settings/native.ts";
  for (const command of [
    "agent_import_registration",
    "agent_cancel_registration",
  ]) {
    assert.deepEqual(
      checkSource(
        native,
        `import { invoke } from '@tauri-apps/api/core'; invoke('${command}')`,
      ),
      [],
    );
    assert.ok(
      checkSource(
        "apps/desktop/src/settings/LocalService.tsx",
        `import { invoke } from '@tauri-apps/api/core'; invoke('${command}')`,
      ).length,
    );
  }
  assert.ok(
    checkSource(
      native,
      `import { invoke } from '@tauri-apps/api/core'; const command = 'agent_import_registration'; invoke(command)`,
    ).length,
  );
  assert.ok(
    checkSource(
      "apps/desktop/src/settings/LocalService.tsx",
      "setInterval(() => {}, 2000);",
    ).length,
  );
  assert.ok(
    checkSource(
      "apps/desktop/src/settings/LocalService.tsx",
      "fetch('https://example.test');",
    ).length,
  );
});

test("only the production main can register the fixed deep-link plugin", () => {
  const source = "fn run() { builder.plugin(tauri_plugin_deep_link::init()); }";
  assert.deepEqual(
    checkRustSources({ "apps/desktop/src-tauri/src/main.rs": source }),
    [],
  );
  assert.ok(
    checkRustSources({
      "apps/desktop/src-tauri/src/composition/navigation.rs": source,
    }).length,
  );
  assert.ok(
    checkRustSources({
      "apps/desktop/src-tauri/src/main.rs":
        "fn run() { builder.plugin(tauri_plugin_deep_link::init(input)); }",
    }).length,
  );
});

test("React JSX rejects executable content, outbound attributes and ambient capability aliases", () => {
  for (const source of [
    'export const View = () => <div dangerouslySetInnerHTML={{__html: "bad"}} />;',
    'export const View = () => <img src="https://example.invalid/a" />;',
    "export const View = () => <iframe />;",
    'export const View = () => <a href="https://example.invalid">link</a>;',
    "export const View = () => <form><input /></form>;",
    'export const View = () => <div {...{dangerouslySetInnerHTML: {__html: "x"}}} />;',
    'const host = globalThis; host.fetch("x");',
    'const host = window; host["fetch"]("x");',
    'const key = "inner" + "HTML"; const x = {}; x[key] = "x";',
    'const key = "constructor"; const x = {}; x[key]("return window")();',
    'const x = {}; x.innerHTML = "x";',
    'const x = {}; x.insertAdjacentHTML("beforeend", "x");',
    'import("@rss-mdm-agent/ai-host");',
    'import x from "node:fs";',
    'import x from "@tauri-apps/api/core";',
    'import x from "@rss-mdm-agent/ai-adapter-codex";',
    'import x from "../../../apps/desktop/src/assembly";',
    'const x = new WebSocket("x");',
    'localStorage.setItem("credential", "x");',
  ])
    assert.ok(checkSource("packages/ui/src/Attack.tsx", source).length, source);
  assert.deepEqual(
    checkSource(
      "packages/ui/src/Safe.tsx",
      'import { Button } from "antd"; export function Safe({text}: {text: string}) { return <Button onClick={() => {}}>{text}</Button>; }',
    ),
    [],
  );
});

test("system appearance and production fixture imports remain narrow capabilities", () => {
  const provider = "packages/ui/src/UiProvider.tsx";
  assert.deepEqual(
    checkSource(provider, 'window.matchMedia("(prefers-color-scheme: dark)");'),
    [],
  );
  for (const source of [
    'window.fetch("x")',
    "const host = window;",
    'window["matchMedia"]("x");',
  ])
    assert.ok(checkSource(provider, source).length, source);
  const main = "apps/desktop/src/main.tsx";
  assert.deepEqual(
    checkSource(
      main,
      'import { fixtureSelected } from "./assembly"; if (import.meta.env.DEV && fixtureSelected) { import("../../../tests/assistant/main"); }',
    ),
    [],
  );
  assert.ok(
    checkSource(main, 'import("../../../tests/assistant/main");').length,
  );
  for (const name of [
    "@rss-mdm-agent/ai-host",
    "@rss-mdm-agent/ai-access",
    "@rss-mdm-agent/ai-adapter-codex",
    "../settings/native",
  ])
    assert.ok(
      checkSource(
        "apps/desktop/src/assistant/View.tsx",
        `import x from "${name}";`,
      ).length,
      name,
    );
});

test("React component resources and element factories cannot bypass presentation limits", () => {
  for (const source of [
    'import { Button } from "antd"; const node = <Button href="https://example.invalid">go</Button>;',
    'import { Image } from "antd"; const node = <Image src="https://example.invalid/pixel" />;',
    'import { createElement } from "react"; createElement("div", {dangerouslySetInnerHTML: {__html: "bad"}});',
    'import { createElement as h } from "react"; h("a", {href:"https://example.invalid"});',
    'import * as React from "react"; React.createElement("iframe", {});',
    'import * as React from "react"; const h = React.createElement; h("a", {});',
    'import { createElement } from "react"; const props = {}; createElement("div", props);',
    'import { createElement } from "react"; createElement("div", {...{dangerouslySetInnerHTML: {__html: "bad"}}});',
    'import { createElement as h } from "react"; const tag = "iframe"; h(tag, {});',
  ])
    assert.ok(checkSource("packages/ui/src/Attack.tsx", source).length, source);
  assert.deepEqual(
    checkSource(
      "packages/ui/src/Safe.tsx",
      'import { createElement } from "react"; createElement("div", {className:"safe"}, "text");',
    ),
    [],
  );
});

test("React namespace destructuring and cloneElement cannot escape props checks", () => {
  for (const source of [
    'import React from "react"; const {createElement:h}=React; export const View=()=>h("img",{src:"https://attacker.invalid/leak"});',
    'import {cloneElement} from "react"; export const View=()=>cloneElement(<div/>,{dangerouslySetInnerHTML:{__html:"<img src=x>"}});',
    'import * as R from "react"; const copy=R; const {createElement:h}=copy; h("img",{src:"x"});',
    'import React from "react"; const {["create"+"Element"]:h}=React; h("img",{src:"x"});',
    'import {cloneElement as clone} from "react"; clone(<div/>, {href:"https://attacker.invalid"});',
  ])
    assert.ok(checkSource("packages/ui/src/Attack.tsx", source).length, source);
});

test("equivalent React default imports and re-exports retain namespace restrictions", () => {
  for (const source of [
    'import {default as React} from "react"; const {createElement}=React; export const View=()=>createElement("img",{src:"https://attacker.invalid/leak"});',
    'import {"default" as R} from "react"; const {createElement:h}=R; h("img",{src:"x"});',
    'export {default as React} from "react";',
    'export {createElement as h} from "react";',
    'export * from "react";',
    'export * as R from "react";',
  ])
    assert.ok(checkSource("packages/ui/src/Attack.tsx", source).length, source);
  assert.deepEqual(
    checkSource(
      "packages/ui/src/Safe.tsx",
      'import {default as React} from "react"; export const View=()=>React.createElement("div",{className:"safe"},"text");',
    ),
    [],
  );
  assert.deepEqual(
    checkSource(
      "packages/ui/src/Safe.tsx",
      'export type {ReactNode} from "react";',
    ),
    [],
  );
});

// Official nodes and component-owned lifecycle need no transport or native capability.
test("React A2UI DOM owner remains unable to acquire network and native authority", () => {
  const file = "apps/desktop/src/assistant/operation-ui/nodes.tsx";
  assert.deepEqual(
    checkSource(
      file,
      'import {createRoot} from "react-dom/client"; import {getValue} from "@a2ui/web_core/v0_9"; const roots = new WeakMap();',
    ),
    [],
  );
  for (const source of [
    'fetch("https://example.invalid")',
    'window.open("x")',
    'import {invoke} from "@tauri-apps/api/core"; invoke("execute")',
    '<iframe src="x"/>',
  ])
    assert.ok(checkSource(file, source).length, source);
  assert.ok(
    checkSource(
      "apps/desktop/src/assistant/Questions.tsx",
      'import {createRoot} from "react-dom/client";',
    ).length,
  );
});

test("retired Vue SFC is rejected before its template can bypass the React parser", () => {
  const source =
    '<template><iframe src="https://untrusted.invalid" /></template><script setup>const value=1;</script>';
  assert.match(
    checkSource("apps/desktop/src/Legacy.vue", source).join("\n"),
    /retired Vue SFC/,
  );
});
