import {
  cpSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import { checkTree, checkSource, checkRustSources } from "./boundaries.mjs";

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
test("UI boundary rejects prohibited imports and rendering even through alternate syntax", () => {
  for (const source of [
    `import x from '@tauri-apps/api/core'`,
    `export * from 'pinia'`,
    `const x = import('../api')`,
    `const x = require('node:fs')`,
    `<template><div v-html="text" /></template>`,
    `interface Item { prNumber: number }`,
    `fetch('https://example.com')`,
  ])
    assert.ok(checkSource("fixture.ts", source).length > 0, source);
  assert.deepEqual(
    checkSource(
      "fixture.ts",
      `import { ref } from 'vue'; export interface Item { id: string; text: string }`,
    ),
    [],
  );
});

test("desktop source and manifests cannot acquire host or network capabilities", () => {
  const file = "apps/desktop/src/fixture.ts";
  for (const source of [
    `import { invoke } from '@tauri-apps/api/core'`,
    `import { store } from './transport'`,
    `fetch('https://example.com')`,
  ])
    assert.ok(checkSource(file, source).length > 0, source);
  assert.deepEqual(
    checkSource(
      file,
      `import { MessageStream } from '@rss-mdm-agent/ui'; import './style.css';`,
    ),
    [],
  );
});
test("assistant only consumes public AI client and bridge; no provider, Host or transport implementation", () => {
  const file = "apps/desktop/src/assistant/controller.ts";
  assert.deepEqual(
    checkSource(
      file,
      `import { ClientError } from '@rss-mdm-agent/ai-client'; import { questions } from '@rss-mdm-agent/ai-ui-bridge';`,
    ),
    [],
  );
  for (const source of [
    `import x from '@agentclientprotocol/sdk'`,
    `import x from '@rss-mdm-agent/ai-contract/testing'`,
    `import x from '@rss-mdm-agent/ai-access'`,
    `import x from '@anthropic-ai/claude-agent-sdk'`,
    `fetch('/send')`,
    `new WebSocket('ws://localhost')`,
    `import x from '../../../../tests/assistant/server.mjs'`,
    `const x = globalThis['fetch']`,
    `const x = globalThis[key]`,
  ])
    assert.ok(checkSource(file, source).length, source);
});
test("desktop features cannot import siblings or escape through the assembly", () => {
  for (const feature of ["assistant", "self-service"]) {
    const file = `apps/desktop/src/${feature}/owner.ts`;
    const sibling = feature === "assistant" ? "self-service" : "assistant";
    for (const source of [
      `import x from '../${sibling}/controller'`,
      `export * from '../${sibling}/controller'`,
      `const x = import('../${sibling}/controller')`,
      `import x from '../App.vue'`,
      `import x from './nested/../../${sibling}/controller'`,
    ])
      assert.ok(checkSource(file, source).length, source);
    assert.deepEqual(checkSource(file, `import x from './controller'`), []);
  }
  assert.deepEqual(
    checkSource(
      "apps/desktop/src/App.vue",
      `<script setup>
    import x from './assistant/controller'; import y from './self-service/controller';
    const identity = () => crypto.randomUUID();
  </script>`,
    ),
    [],
  );
});
test("alternative HTML sinks are rejected in Vue templates and scripts", () => {
  for (const source of [
    `<script setup>document.body.insertAdjacentHTML('beforeend', input)</script>`,
    `<script setup>document.write(input)</script>`,
    `<script setup>new DOMParser().parseFromString(input, 'text/html')</script>`,
    `<template><button @click="document.write(input)">Send</button></template>`,
    `<script setup>node['innerHTML'] = input</script>`,
  ])
    assert.ok(
      checkSource("packages/ui/src/fixture.vue", source).length > 0,
      source,
    );
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
    assert.ok(errors.includes("UI production dependencies must be Vue only"));
    assert.ok(
      errors.includes(
        "desktop production dependencies must be UI, public AI client/bridge, Vue and pinned Tauri core only",
      ),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("presentation capabilities reject global aliases, computed access and outbound templates", () => {
  for (const source of [
    `globalThis['fetch']('https://example.com')`,
    `const send = navigator.sendBeacon; send('https://example.com', 'x')`,
    `const host = window; host['__TAURI_INTERNALS__'].invoke('run')`,
    `const { fetch: send } = globalThis; send('https://example.com')`,
    `const key = 'fe' + 'tch'; globalThis[key]('https://example.com')`,
    `globalThis[\`fetch\`]('https://example.com')`,
    `new Image().src = 'https://example.com'`,
    `import network = require('node:http')`,
    `<template><div v-if="true"><a href="https://example.com">out</a></div></template>`,
    `const value = {}; const key = 'con' + 'structor'; value[key]('return window')()`,
    `<template><a href="https://example.com">out</a></template>`,
    `<template><form action="https://example.com"><button>send</button></form></template>`,
    `<template><img :src="url" /></template>`,
    `<template><button @click="window['open'](url)">out</button></template>`,
    `<template><component :is="tag" v-bind="attrs" /></template>`,
    `const doc = document; doc.createElement('script')`,
    `const ctor = value['con' + 'structor']; ctor('return window')()`,
  ])
    assert.ok(
      checkSource(
        "packages/ui/src/fixture.vue",
        source.startsWith("<") ? source : `<script setup>${source}</script>`,
      ).length,
      source,
    );
});

test("each host boundary mutation independently fails the tree scan", () => {
  const dir = mkdtempSync(join(tmpdir(), "ui-host-boundaries-"));
  const mutations = [
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
    `import { invoke } from '@tauri-apps/api/core'; const name = 'self_service_snapshot'; invoke(name)`,
    `import { invoke as call } from '@tauri-apps/api/core'; call('self_service_snapshot')`,
    `import { invoke } from '@tauri-apps/api/core'; export const call = invoke`,
    `import * as native from '@tauri-apps/api/core'; native.invoke('run_shell')`,
  ])
    assert.ok(checkSource(file, source).length, source);
  assert.deepEqual(
    checkSource(
      file,
      `import { invoke } from '@tauri-apps/api/core'; invoke('self_service_snapshot')`,
    ),
    [],
  );
});

test("trusted execution details share only the frozen-origin view across feature directories", () => {
  const file = "apps/desktop/src/assistant/ExecutionDetails.vue";
  const source = (name) =>
    `<script setup>import View from "${name}";</script><template><View /></template>`;
  assert.deepEqual(
    checkSource(file, source("../self-service/RequestOrigin.vue")),
    [],
  );
  for (const name of [
    "../self-service/native",
    "../self-service/controller",
    "../self-service/TaskDetail.vue",
  ])
    assert.ok(checkSource(file, source(name)).length);
});
