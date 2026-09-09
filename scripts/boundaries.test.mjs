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
