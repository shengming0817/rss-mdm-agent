import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  assertFrontendClosure,
  verifyFrontendClosure,
} from "./frontend-closure.mjs";
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const source = {
  head: "a".repeat(40),
  sourceSha256: "b".repeat(64),
  cargoLockSha256: "c".repeat(64),
  pnpmLockSha256: "d".repeat(64),
};
test("actual emitted module inventory rejects retired frameworks and development owners, including lazy chunks", () => {
  assert.doesNotThrow(() =>
    assertFrontendClosure(
      [
        "apps/desktop/src/main.tsx",
        "node_modules/.pnpm/react@19.3.1/node_modules/react/index.js",
      ],
      { "index.html": Buffer.from('<div id="root"></div>') },
    ),
  );
  for (const id of [
    "node_modules/.pnpm/vue@3/node_modules/vue/index.js",
    "node_modules/.pnpm/@vue+shared@3/node_modules/@vue/shared/index.js",
    "node_modules/reka-ui/dist/index.js",
    "node_modules/vue-demi/index.js",
    "apps/desktop/src/Legacy.vue",
    "tests/assistant/desktop.tsx",
    "docs/product/ui-ux/react-antd-desktop.html",
  ])
    assert.throws(
      () => assertFrontendClosure(["apps/desktop/src/main.tsx", id], {}),
      /non-production frontend module/,
    );
  for (const marker of ["/__fixture/connect", "隔离样本与验证工具"])
    assert.throws(
      () =>
        assertFrontendClosure(["apps/desktop/src/main.tsx"], {
          "assets/lazy.js": Buffer.from(marker),
        }),
      /development fixture/,
    );
});
test("fixed frontend evidence detects substituted, added and removed output or different source without loading an app", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "rss-frontend-closure-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  mkdirSync(join(directory, "assets"));
  const write = () => {
    writeFileSync(join(directory, "index.html"), "fixed index");
    writeFileSync(join(directory, "assets/app.js"), "fixed code");
    writeFileSync(
      join(directory, "frontend-closure.json"),
      JSON.stringify({
        version: 1,
        owner: "desktop",
        source,
        modules: ["apps/desktop/src/main.tsx"],
        files: {
          "assets/app.js": sha("fixed code"),
          "index.html": sha("fixed index"),
        },
      }),
    );
  };
  write();
  assert.equal(
    verifyFrontendClosure(directory, source).source.head,
    source.head,
  );
  writeFileSync(
    join(directory, "index.d.ts"),
    "export declare const value: string;",
  );
  assert.equal(verifyFrontendClosure(directory, source).owner, "desktop");
  assert.throws(
    () => verifyFrontendClosure(directory, { ...source, head: "e".repeat(40) }),
    /source differs/,
  );
  writeFileSync(join(directory, "assets/app.js"), "replacement");
  assert.throws(() => verifyFrontendClosure(directory), /output differs/);
  write();
  writeFileSync(join(directory, "assets/stale-vue.js"), "old output");
  assert.throws(() => verifyFrontendClosure(directory), /output differs/);
  rmSync(join(directory, "assets/stale-vue.js"));
  rmSync(join(directory, "assets/app.js"));
  assert.throws(() => verifyFrontendClosure(directory), /output differs/);
});
