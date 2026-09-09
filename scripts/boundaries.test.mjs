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
