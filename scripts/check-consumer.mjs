import {
  mkdtempSync,
  writeFileSync,
  readdirSync,
  rmSync,
  readFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dir = mkdtempSync(join(tmpdir(), "rss-ui-consumer-"));
function run(command, args, cwd = dir) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0)
    throw new Error(
      `${command} ${args.join(" ")} failed: ${result.error ?? result.status}`,
    );
}
try {
  run("pnpm", [
    "--dir",
    join(root, "packages/ui"),
    "pack",
    "--pack-destination",
    dir,
  ]);
  const archive = readdirSync(dir).find((f) => f.endsWith(".tgz"));
  if (!archive) throw new Error("UI archive missing");
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name: "isolated-ui-consumer",
      private: true,
      type: "module",
      dependencies: {
        "@rss-mdm-agent/ui": `file:./${archive}`,
        vue: versions.vue,
      },
      devDependencies: Object.fromEntries(
        ["typescript", "vue-tsc", "vite", "@vitejs/plugin-vue"].map((name) => [
          name,
          versions[name],
        ]),
      ),
    }),
  );
  writeFileSync(
    join(dir, "pnpm-workspace.yaml"),
    "packages: []\nallowBuilds:\n  esbuild: true\n",
  );
  writeFileSync(
    join(dir, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        module: "ESNext",
        moduleResolution: "Bundler",
        strict: true,
        skipLibCheck: true,
        lib: ["ES2022", "DOM"],
        types: ["vite/client"],
      },
      include: ["main.ts", "App.vue"],
    }),
  );
  writeFileSync(
    join(dir, "index.html"),
    '<html><body><div id="app"></div><script type="module" src="/main.ts"></script></body></html>',
  );
  writeFileSync(
    join(dir, "main.ts"),
    `import { createApp } from 'vue'; import App from './App.vue'; import '@rss-mdm-agent/ui/style.css'; createApp(App).mount('#app');`,
  );
  writeFileSync(
    join(dir, "App.vue"),
    `<script setup lang="ts">
import { AppShell, NavigationList, SplitPane, MessageStream, MessageComposer, StatusList, type MessageItem, type NavigationItem, type StatusItem } from '@rss-mdm-agent/ui';
const messages: MessageItem[] = [{ id: 'm', kind: 'assistant', text: '<script>not executable</scr' + 'ipt>' }];
const nav: NavigationItem[] = [{ id: 'home', label: 'Home' }];
const statuses: StatusItem[] = [{ id: 's', label: 'Sample', message: 'Ready', tone: 'neutral' }];
</script><template><AppShell><template #navigation><NavigationList :items="nav" active-id="home" /></template><SplitPane><template #top><MessageStream :items="messages" /></template><template #bottom><MessageComposer model-value="" /></template></SplitPane><template #status><StatusList :items="statuses" /></template></AppShell></template>`,
  );
  writeFileSync(
    join(dir, "vite.config.js"),
    `import { defineConfig } from 'vite'; import vue from '@vitejs/plugin-vue'; export default defineConfig({ plugins: [vue()] });`,
  );
  writeFileSync(
    join(dir, "smoke.mjs"),
    `import assert from 'node:assert/strict'; import { createSSRApp, h } from 'vue'; import { renderToString } from 'vue/server-renderer'; import { MessageStream } from '@rss-mdm-agent/ui'; const html = await renderToString(createSSRApp({ render: () => h(MessageStream, { items: [{ id: 'm', kind: 'assistant', text: '<img onerror=alert(1)>' }] }) })); assert.ok(html.includes('&lt;img')); assert.ok(!html.includes('<img')); console.log('Packed public API renders inert text');`,
  );
  run("pnpm", ["install", "--offline"]);
  run("pnpm", ["exec", "vue-tsc", "--noEmit"]);
  run("pnpm", ["exec", "vite", "build"]);
  run("node", ["smoke.mjs"]);
  console.log(
    "Isolated tarball consumer: types, CSS, build and rendering passed",
  );
} finally {
  rmSync(dir, { recursive: true, force: true });
}
