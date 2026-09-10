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
import { checkTree, checkSource } from "./boundaries.mjs";

test("UI and desktop satisfy the dependency and permission boundaries", () => {
  assert.deepEqual(checkTree(), []);
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
        "desktop production dependencies must be UI and Vue only",
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
      (s) => s.replace('"permissions": []', '"permissions": ["core:default"]'),
    ],
    [
      "apps/desktop/src-tauri/capabilities/main.json",
      (s) => s.replace('"windows": ["main"]', '"windows": ["*"]'),
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
