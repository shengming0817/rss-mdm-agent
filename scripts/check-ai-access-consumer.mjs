import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  cpSync,
  rmSync,
  existsSync,
} from "node:fs";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { chromium } from "playwright-core";
const root = resolve(dirname(fileURLToPath(import.meta.url)), ".."),
  dir = mkdtempSync(join(tmpdir(), "rss-ai-access-consumer-"));
const run = (command, args) => {
  const r = spawnSync(command, args, {
    cwd: dir,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (r.status !== 0) throw new Error(`${command} failed (${r.status})`);
};
let browser, server, service;
try {
  const packages = ["ai-contract", "ai-access", "ai-client", "ai-ui-bridge"];
  for (const name of packages)
    run("pnpm", [
      "--dir",
      join(root, "packages", name),
      "pack",
      "--pack-destination",
      dir,
    ]);
  const dependencies = Object.fromEntries(
    packages.map((name) => [
      `@rss-mdm-agent/${name}`,
      `file:./${readdirSync(dir).find((f) => f === `rss-mdm-agent-${name}-0.1.0.tgz`)}`,
    ]),
  );
  Object.assign(dependencies, {
    vue: "3.5.38",
    "@agentclientprotocol/sdk": "1.4.0",
  });
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name: "isolated-ai-access-consumer",
      private: true,
      type: "module",
      dependencies,
      devDependencies: { vite: "6.4.3", "@vitejs/plugin-vue": "5.2.1" },
    }),
  );
  writeFileSync(
    join(dir, "pnpm-workspace.yaml"),
    `packages: []\nallowBuilds:\n  esbuild: true\noverrides: ${JSON.stringify(Object.fromEntries(packages.map((name) => [`@rss-mdm-agent/${name}`, dependencies[`@rss-mdm-agent/${name}`]])))}\n`,
  );
  cpSync(join(root, "tests/ai-access/consumer"), dir, { recursive: true });
  writeFileSync(
    join(dir, "index.html"),
    '<html><body><div id="app"></div><script type="module" src="/main.js"></script></body></html>',
  );
  writeFileSync(
    join(dir, "vite.config.mjs"),
    `import {defineConfig} from 'vite'; import vue from '@vitejs/plugin-vue';
export default defineConfig({plugins:[{name:'forbid-server-imports',enforce:'pre',resolveId(id){if(id.startsWith('node:')||['fs','crypto','path','stream','buffer','child_process'].includes(id))throw new Error('Browser imported server dependency: '+id);}},vue({template:{compilerOptions:{isCustomElement:tag=>tag.startsWith('a2ui-')}}})],build:{target:'es2022'}});`,
  );
  run("pnpm", ["install", "--offline", "--ignore-scripts"]);
  run("node", ["standard.mjs"]);
  run("pnpm", ["exec", "vite", "build"]);
  writeFileSync(
    join(dir, "consumer-exports.mjs"),
    'export {createAccessService} from "@rss-mdm-agent/ai-access"; export {FakeHost,MemorySessionStore,fixtureCaller,seedSurface,surfaceCommit,unwrap} from "@rss-mdm-agent/ai-contract/testing"; export {channelStream} from "@rss-mdm-agent/ai-client";',
  );
  const {
    createAccessService,
    FakeHost,
    MemorySessionStore,
    fixtureCaller,
    seedSurface,
    surfaceCommit,
    unwrap,
    channelStream,
  } = await import(pathToFileURL(join(dir, "consumer-exports.mjs")).href);
  const store = new MemorySessionStore(),
    seeded = await seedSurface(store),
    host = new FakeHost(store);
  service = createAccessService({
    host,
    now: () => 0,
    sessionOptions: {
      provider: "fake",
      config: { id: "cfg", revision: "1" },
      accountRef: "a",
      profile: "conversation",
    },
  });
  let receive, disconnect;
  const outgoing = [];
  service.connect(
    channelStream({
      send: async (message) => {
        outgoing.push(message);
      },
      listen: (r, d) => {
        receive = r;
        disconnect = d;
        return () => {};
      },
    }),
    fixtureCaller,
  );
  let surface = seeded.surface;
  async function change(operation) {
    const message =
      operation === "data"
        ? {
            version: "v0.9.1",
            updateDataModel: {
              surfaceId: surface.surfaceId,
              path: "/question",
              value: '<img src=x onerror="window.injected=true"> Safe text',
            },
          }
        : operation === "components"
          ? {
              version: "v0.9.1",
              updateComponents: {
                surfaceId: surface.surfaceId,
                components: [
                  { id: "label", component: "Text", text: "Confirm answer" },
                ],
              },
            }
          : {
              version: "v0.9.1",
              deleteSurface: { surfaceId: surface.surfaceId },
            };
    surface = {
      ...surface,
      revision: surface.revision + 1,
      status: operation === "delete" ? "deleted" : "active",
      messages: [...surface.messages, message],
    };
    const current = unwrap(await store.session(seeded.session.namespace));
    unwrap(
      await store.commit(
        surfaceCommit(current, surface, seeded.interaction.commandId),
      ),
    );
    host.notify(current.namespace);
  }
  server = createServer(async (req, res) => {
    try {
      if (req.url === "/send") {
        let data = "";
        for await (const chunk of req) {
          data += chunk;
          if (data.length > 262144) throw new Error("budget");
        }
        receive(JSON.parse(data));
        res.end("ok");
      } else if (req.url === "/receive") {
        res.setHeader("content-type", "application/json");
        res.end(JSON.stringify(outgoing.splice(0)));
      } else if (req.url.startsWith("/change/")) {
        await change(req.url.slice(8));
        res.end("ok");
      } else {
        const file = join(
          dir,
          "dist",
          req.url === "/" ? "index.html" : req.url.split("?")[0],
        );
        if (!file.startsWith(join(dir, "dist")) || !existsSync(file)) {
          res.statusCode = 404;
          res.end();
          return;
        }
        res.setHeader(
          "content-type",
          file.endsWith(".js")
            ? "text/javascript"
            : file.endsWith(".css")
              ? "text/css"
              : "text/html",
        );
        res.end(readFileSync(file));
      }
    } catch (error) {
      res.statusCode = 500;
      res.end("fixture failure");
      console.error(error);
    }
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const executablePath =
    process.env.AI_BROWSER_PATH ??
    (process.platform === "darwin"
      ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
      : undefined);
  browser = await chromium.launch({
    ...(executablePath ? { executablePath } : {}),
    headless: true,
  });
  const page = await browser.newPage();
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.getByText("Choose an option", { exact: true }).waitFor();
  await page.locator("#fail-renderer").click();
  await page
    .locator("p[role=status]")
    .filter({ hasText: "could not load" })
    .waitFor();
  await page.locator("#fail-renderer").click();
  await page.getByText("Choose an option", { exact: true }).waitFor();
  await page.locator("input").fill("browser answer");
  await page.evaluate(() => fetch("/change/data", { method: "POST" }));
  await page
    .getByText('<img src=x onerror="window.injected=true"> Safe text', {
      exact: true,
    })
    .waitFor();
  if (await page.evaluate(() => Boolean(window.injected)))
    throw new Error("HTML executed");
  await page.evaluate(() => fetch("/change/components", { method: "POST" }));
  await page
    .getByRole("button", { name: "Confirm answer", exact: true })
    .waitFor();
  await page.locator("#toggle").click();
  await page.locator("a2ui-surface").waitFor({ state: "detached" });
  await page.locator("#toggle").click();
  await page
    .getByRole("button", { name: "Confirm answer", exact: true })
    .waitFor();
  await page.locator("input").fill("browser answer");
  await page
    .getByRole("button", { name: "Confirm answer", exact: true })
    .click();
  await page.waitForFunction(
    () => document.querySelector("#receipts")?.textContent === "1",
  );
  const snapshot = unwrap(
    await store.snapshotPage(seeded.session.namespace, { limit: 64 }),
  );
  const answer = snapshot.commands.find(
    (row) => row.command.input.type === "respond",
  );
  if (answer?.command.input.answer.answer !== "browser answer")
    throw new Error("renderer action did not preserve entered data");
  await page.evaluate(() => fetch("/change/delete", { method: "POST" }));
  await page.locator("a2ui-surface").waitFor({ state: "detached" });
  if (failures.length) throw new Error(failures.join("; "));
  console.log(
    `PASS isolated Vue + official A2UI browser ${await browser.version()}: create/components/data/delete, literal HTML, remount, action`,
  );
  disconnect();
} finally {
  await browser?.close();
  service?.close();
  if (server) await new Promise((r) => server.close(r));
  if (process.env.AI_KEEP_CONSUMER) console.log(`Consumer retained: ${dir}`);
  else rmSync(dir, { recursive: true, force: true });
}
