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
      devDependencies: {
        vite: "6.4.3",
        "@vitejs/plugin-vue": "5.2.1",
        typescript: "5.6.2",
      },
    }),
  );
  // Pin the offline browser toolchain to the workspace lock; refreshed registry metadata must not select an uncached/new native binary.
  writeFileSync(
    join(dir, "pnpm-workspace.yaml"),
    `packages: []\nallowBuilds:\n  esbuild: true\noverrides: ${JSON.stringify({ rollup: "4.63.1", ...Object.fromEntries(packages.map((name) => [`@rss-mdm-agent/${name}`, dependencies[`@rss-mdm-agent/${name}`]])) })}\n`,
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
  run("pnpm", [
    "exec",
    "tsc",
    "--noEmit",
    "--strict",
    "--target",
    "ES2022",
    "--module",
    "NodeNext",
    "--moduleResolution",
    "NodeNext",
    "--skipLibCheck",
    "public-types.ts",
  ]);
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
  // Real Vue/Lit lifecycle seam with deterministic authoritative projection stimuli.
  await page.evaluate(() => {
    const api = window.consumer,
      listeners = new Set();
    const state = {
      view: api.runtime.getSession("session-1"),
      now: 0,
      attempts: [],
      errors: [],
    };
    const container = document.createElement("div");
    container.id = "interaction-fixture";
    document.body.append(container);
    const runtime = {
      getSession: () => structuredClone(state.view),
      observe(fn) {
        listeners.add(fn);
        return () => listeners.delete(fn);
      },
      action(request) {
        state.attempts.push(request);
        return new Promise((resolve, reject) => {
          state.resolve = resolve;
          state.reject = reject;
        });
      },
    };
    state.update = (patch = {}) => {
      Object.assign(state.view.interactions["question-1"], patch);
      for (const fn of listeners) fn(structuredClone(state.view));
    };
    state.app = api.createApp(api.RuntimeSurface, {
      runtime,
      sessionId: "session-1",
      instanceId: "surface-instance-1",
      now: () => state.now,
      onError: (error) =>
        state.errors.push({ code: error.code, failure: error.failure }),
    });
    state.app.mount(container);
    api.interactionTest = state;
  });
  const fixture = page.locator("#interaction-fixture");
  await fixture.locator("a2ui-surface").waitFor();
  // Deadline is inclusive and expires without any server notification or rerender.
  await page.evaluate(() => {
    const s = window.consumer.interactionTest;
    s.now = 100;
    s.update();
  });
  if (await fixture.locator("a2ui-surface").evaluate((e) => e.inert))
    throw new Error("inclusive interaction deadline disabled too soon");
  await page.evaluate(() => {
    window.consumer.interactionTest.now = 101;
  });
  await page.waitForFunction(
    () => document.querySelector("#interaction-fixture a2ui-surface")?.inert,
  );
  if (
    await fixture
      .getByRole("button", { name: "Retry response", exact: true })
      .count()
  )
    throw new Error("expired question offered retry");
  await page.evaluate(() => {
    const s = window.consumer.interactionTest;
    s.now = 0;
    s.update();
  });
  await fixture.locator("a2ui-surface").getByRole("button").click();
  await page.waitForFunction(
    () => window.consumer.interactionTest.attempts.length === 1,
  );
  if (
    (await page.evaluate(
      () => window.consumer.interactionTest.attempts[0].expiresAtMs,
    )) !== 100
  )
    throw new Error("action extended the authoritative interaction deadline");
  // A response lost before the other client's winning event may offer retry only until that event arrives.
  await page.evaluate(() =>
    window.consumer.interactionTest.reject(
      new window.consumer.ClientError("transport_failed"),
    ),
  );
  await fixture
    .getByRole("button", { name: "Retry response", exact: true })
    .waitFor();
  await page.evaluate(() =>
    window.consumer.interactionTest.update({
      status: "answered",
      responseCommandId: "other-client",
    }),
  );
  await fixture
    .getByRole("button", { name: "Retry response", exact: true })
    .waitFor({ state: "detached" });
  if (await fixture.locator(".rss-ai-surface-error").count())
    throw new Error("losing response retained an obsolete error");
  // A late rejection after callback loss cannot recreate a retry affordance.
  await page.evaluate(() =>
    window.consumer.interactionTest.update({
      status: "pending",
      responseCommandId: undefined,
    }),
  );
  await fixture.locator("a2ui-surface").getByRole("button").click();
  await page.waitForFunction(
    () => window.consumer.interactionTest.attempts.length === 2,
  );
  await page.evaluate(() => {
    const s = window.consumer.interactionTest;
    s.update({ status: "unavailable" });
    s.reject(new window.consumer.ClientError("already_answered"));
  });
  await page.waitForFunction(
    () => document.querySelector("#interaction-fixture a2ui-surface")?.inert,
  );
  if (
    await fixture
      .getByRole("button", { name: "Retry response", exact: true })
      .count()
  )
    throw new Error("late response failure revived an invalid callback");
  // A terminal RPC failure is also closed when the winning notification has not arrived yet.
  await page.evaluate(() =>
    window.consumer.interactionTest.update({ status: "pending" }),
  );
  await fixture.locator("a2ui-surface").getByRole("button").click();
  await page.waitForFunction(
    () => window.consumer.interactionTest.attempts.length === 3,
  );
  await page.evaluate(() =>
    window.consumer.interactionTest.reject(
      new window.consumer.ClientError("already_answered"),
    ),
  );
  await fixture.locator(".rss-ai-surface-error").waitFor();
  if (
    await fixture
      .getByRole("button", { name: "Retry response", exact: true })
      .count()
  )
    throw new Error(
      "terminal rejection offered retry without a winning notification",
    );
  const reported = await page.evaluate(() =>
    window.consumer.interactionTest.errors.at(-1),
  );
  if (
    reported.code !== "action_rejected" ||
    reported.failure !== "already_answered"
  )
    throw new Error("renderer did not expose closed error codes");
  await page.evaluate(() => {
    window.consumer.interactionTest.app.unmount();
    document.getElementById("interaction-fixture").remove();
  });
  await page.locator("#fail-renderer").click();
  await page
    .locator("p[role=status]")
    .filter({ hasText: "could not load" })
    .waitFor();
  await page.getByRole("button", { name: "Retry card", exact: true }).click();
  await page.getByText("Choose an option", { exact: true }).waitFor();
  await page.locator("#fail-replace").click();
  await page
    .locator("p[role=status]")
    .filter({ hasText: "cannot be displayed" })
    .waitFor();
  await page.evaluate(() => window.consumer.runtime.restore("session-1"));
  await page.getByRole("button", { name: "Retry card", exact: true }).click();
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
  await page.locator("#lose-response").click();
  await page
    .getByRole("button", { name: "Confirm answer", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Retry response", exact: true })
    .waitFor();
  if ((await page.locator("#receipts").textContent()) !== "0")
    throw new Error("lost response reported as received");
  await page
    .getByRole("button", { name: "Retry response", exact: true })
    .click();
  await page.waitForFunction(
    () => document.querySelector("#receipts")?.textContent === "1",
  );
  const attempts = (await page.locator("#attempts").textContent()).split(",");
  if (attempts.length !== 2 || attempts[0] !== attempts[1])
    throw new Error("retry changed command identity");
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
    `PASS isolated Vue + official A2UI browser ${await browser.version()}: create/components/data/delete, literal HTML, load/replace recovery, remount, exact action retry, authoritative expiry, competing answer and late rejection`,
  );
  disconnect();
} finally {
  await browser?.close();
  await service?.close();
  if (server) await new Promise((r) => server.close(r));
  if (process.env.AI_KEEP_CONSUMER) console.log(`Consumer retained: ${dir}`);
  else rmSync(dir, { recursive: true, force: true });
}
