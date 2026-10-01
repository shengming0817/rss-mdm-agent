// Daily macOS golden path: root pnpm dev -> production main -> real WKWebView.
// ref: webdriverio/desktop-mobile packages/tauri-plugin-webdriver@1.4.0;
// webdriverio/webdriverio packages/webdriverio/src/index.ts@v9.32.0.
import assert from "node:assert/strict";
import { unwrap } from "../packages/ai-contract/dist/testing/index.js";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { randomBytes } from "node:crypto";
import { createServer } from "node:net";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { remote } from "webdriverio";
import { startModelFixture } from "../tests/desktop/model-fixture.mjs";
import {
  sourceEvidence,
  sha256,
  waitForAppearance,
} from "./native-evidence.mjs";
import { developmentFingerprint } from "./desktop-dev-runtime.mjs";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";

const visualOnly = process.argv.includes("--visual");
const baseline = process.argv.includes("--baseline");
let visualFixture;
const root = fileURLToPath(new URL("../", import.meta.url));
const reports = join(root, ".local-ci-runs");
const nonce = randomBytes(16).toString("hex");
mkdirSync(reports, { recursive: true });
const result = {
  status: "running",
  mode: "pnpm-dev-main",
  modelFixture: true,
  credentials: "synthetic-loopback-only",
  provider: "codex-0.155.0",
  executor:
    "installed execution service; legacy journal assertions require #2593 migration",
  keychain: "isolated noninteractive macOS file keychain",
  messageInput:
    "visible product submit button; physical Return delivery unverified on this interactive desktop",
  checks: [],
};
for (const name of [
  "desktop-native-narrow.png",
  "desktop-native-navigation.png",
  "desktop-native-navigation-narrow.png",
  "desktop-native-failed.png",
  "desktop-native-context-narrow.png",
  "desktop-native-context-wide.png",
  "desktop-native.log",
])
  rmSync(join(reports, name), { force: true });
const writeReport = () => {
  const path = join(reports, "desktop-native.json");
  writeFileSync(path + ".tmp", JSON.stringify(result, null, 2));
  renameSync(path + ".tmp", path);
};
writeReport();
let directory,
  child,
  exited,
  browser,
  fixture,
  receipt,
  hostStatus,
  logs = "",
  stage = "preflight",
  spawnError,
  keychainState;
const systemKeychains = () =>
  ["default-keychain", "list-keychains"].map((command) =>
    execFileSync("/usr/bin/security", [command, "-d", "user"], {
      encoding: "utf8",
      timeout: 10000,
    }).trim(),
  );
const signalRoot = (signal) => {
  if (!child?.pid) return;
  try {
    process.kill(-child.pid, signal);
  } catch (error) {
    if (!["ESRCH", "EPERM"].includes(error.code)) throw error;
  }
};
const rootAlive = () => {
  if (!child?.pid) return false;
  try {
    process.kill(-child.pid, 0);
    return true;
  } catch (error) {
    if (error.code === "EPERM") return true;
    if (error.code !== "ESRCH") throw error;
    return false;
  }
};
let cancelled = false;
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => {
    cancelled = true;
    spawnError = new Error("native acceptance cancelled");
    // The dev wrapper owns Tauri/Vite/main and their bounded process-group shutdown.
    signalRoot("SIGTERM");
  });
const mark = (value) => {
  stage = value;
  result.stage = value;
  writeReport();
  console.log(`[native] ${value}`);
};
const wait = async (check, timeout = 60000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (spawnError) throw spawnError;
    if (fixture?.failure) throw fixture.failure;
    const value = await check();
    if (value) return value;
    await delay(100);
  }
  throw Error(`native timeout at ${stage}`);
};
const script = (source) =>
  execFileSync("/usr/bin/osascript", ["-e", source], {
    encoding: "utf8",
    timeout: 10000,
  }).trim();
const native = (source) =>
  script(
    `tell application "System Events"\n tell (first application process whose unix id is ${receipt.pid})\n set frontmost to true\n if (count of windows) > 0 then perform action "AXRaise" of window 1\n repeat 10 times\n if frontmost then exit repeat\n set frontmost to true\n delay 0.1\n end repeat\n delay 0.2\n ${source}\n end tell\nend tell`,
  );
const key = (code, shift = false) => {
  assert.equal(
    native("get frontmost"),
    "true",
    "native application must own foreground keyboard focus",
  );
  native(`key code ${code}${shift ? " using shift down" : ""}`);
};
const menu = (label) =>
  native(
    `click menu item "${label}" of menu 1 of menu bar item "RSS MDM Agent" of menu bar 1`,
  );
const click = async (name, scope = "") => {
  const el = await wait(async () => {
    for (const candidate of await browser.$$(
      `${scope}//button[normalize-space(.)=${JSON.stringify(name)} or @aria-label=${JSON.stringify(name)}]`,
    ))
      if (
        (await browser.execute(
          (el) =>
            el.isConnected &&
            el.getClientRects().length > 0 &&
            !el.closest("[inert]"),
          candidate,
        )) &&
        (await candidate.isEnabled())
      )
        return candidate;
  });
  await el.click();
};
const visibleText = async (text) =>
  browser.execute((value) => document.body.innerText.includes(value), text);
const text = async (value) => wait(() => visibleText(value));
const field = (label) =>
  browser.$(
    `//form[contains(@class,'connection-form')]//label[starts-with(normalize-space(.),${JSON.stringify(label)})]/*[self::input or self::select]`,
  );
const navigate = async (name) => {
  await wait(() =>
    browser.execute(
      () =>
        innerWidth < 900 ===
        !!document.querySelector('[aria-label="打开主导航"]'),
    ),
  );
  if (await browser.execute(() => innerWidth < 900))
    await browser.$('[aria-label="打开主导航"]').click();
  await click(name);
  await browser.execute(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
};
const prompt = async (value) => {
  await browser.$(".composer textarea").click();
  await browser.$(".composer textarea").setValue(value);
  assert.equal(
    await browser.execute(
      () =>
        document.activeElement === document.querySelector(".composer textarea"),
    ),
    true,
    "composer must own DOM focus before submission",
  );
  await wait(() => browser.$('.composer button[type="submit"]').isEnabled());
  // Send through the visible product action. Physical Return delivery depends on the
  // interactive desktop's input source/foreground; do not resend an ambiguous command.
  await browser.$('.composer button[type="submit"]').click();
  await wait(() =>
    browser.execute(
      (value) => document.querySelector(".composer textarea").value !== value,
      value,
    ),
  );
};
const selectValue = async (element, value) => {
  // The embedded driver cannot reliably select a macOS native option popup.
  // Configure through the actual DOM change event; Enter/Tab/Escape remain native below.
  await browser.execute(
    (select, selected) => {
      select.value = selected;
      select.dispatchEvent(new Event("change", { bubbles: true }));
    },
    element,
    value,
  );
  await wait(async () => (await element.getValue()) === value);
};
const permission = async (allow) => {
  await wait(async () => {
    assert.equal(
      await visibleText("AI Host 已退出"),
      false,
      "AI Host exited before permission",
    );
    return browser.$('[aria-label="AI 工具权限请求"]').isDisplayed();
  });
  const el = await browser.$(
    `//section[@aria-label='AI 工具权限请求']//button[strong[contains(.,'${allow ? "允许一次" : "拒绝一次"}')]]`,
  );
  await el.click();
};
const dbRead = (file, action) => {
  const db = new DatabaseSync(join(directory, file), {
    readOnly: true,
    timeout: 1000,
  });
  try {
    return action(db);
  } finally {
    db.close();
  }
};
const task = () =>
  dbRead("execution.sqlite", (db) => {
    const row = db
      .prepare("SELECT plan,snapshot FROM executions WHERE request_id=?")
      .get("native-golden-install");
    return (
      row && {
        plan: JSON.parse(Buffer.from(row.plan).toString()),
        snapshot: JSON.parse(Buffer.from(row.snapshot).toString()),
      }
    );
  });
try {
  result.source = sourceEvidence(root);
  assert.equal(process.platform, "darwin");
  assert.equal(process.arch, "arm64");
  keychainState = systemKeychains();
  assert.equal(
    execFileSync(
      "/usr/bin/swift",
      [
        "-e",
        'import CoreGraphics; let s = CGSessionCopyCurrentDictionary() as? [String: Any]; print((s?["CGSSessionScreenIsLocked"] as? Bool == true || s == nil) ? "locked" : "unlocked")',
      ],
      { encoding: "utf8" },
    ).trim(),
    "unlocked",
    "unlock the macOS desktop before native keyboard acceptance",
  );
  assert.equal(
    script('tell application "System Events" to get UI elements enabled'),
    "true",
    "System Events accessibility permission is required",
  );
  const packageManifest = JSON.parse(readFileSync(join(root, "package.json")));
  assert.equal(result.source.node, `v${packageManifest.engines.node}`);
  assert.equal(
    result.source.pnpm,
    packageManifest.packageManager.split("@")[1],
  );
  fixture = await startModelFixture();
  directory = realpathSync(mkdtempSync("/tmp/rss-native-"));
  const reserved = createServer();
  await new Promise((resolve) => reserved.listen(0, "127.0.0.1", resolve));
  const port = reserved.address().port;
  await new Promise((resolve) => reserved.close(resolve));
  // The native fixture owns an isolated Vite port; another worktree's preview must not be consumed or stopped.
  const webReserved = createServer();
  await new Promise((resolve) => webReserved.listen(0, "127.0.0.1", resolve));
  let webPort = webReserved.address().port;
  await new Promise((resolve) => webReserved.close(resolve));
  if (visualOnly) {
    const { startFixture } = await import("../tests/assistant/server.mjs");
    visualFixture = await startFixture();
    await visualFixture.seed(2);
    webPort = Number(new URL(visualFixture.url).port);
    result.mode = "pnpm-dev-main-visual";
    result.provider = "FakeHost renderer fixture";
    result.executor =
      "none; read-only sample resources, Rust-generated execution projections";
    result.productionExecution =
      "unverified; installed execution service unavailable in baseline";
  }
  const baseConfig = JSON.parse(
    readFileSync(join(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
  );
  const nativeConfig = {
    build: {
      devUrl: visualFixture?.url ?? `http://127.0.0.1:${webPort}`,
      beforeDevCommand: visualOnly ? "" : `pnpm dev:web --port ${webPort}`,
    },
    app: {
      security: {
        devCsp: baseConfig.app.security.devCsp.replaceAll(
          ":1420",
          `:${webPort}`,
        ),
      },
    },
  };
  result.devWebPort = webPort;
  const env = {
    ...process.env,
    RSS_NATIVE_E2E_NONCE: nonce,
    RSS_NATIVE_E2E_PORT: String(port),
    RSS_NATIVE_E2E_WEB_PORT: String(webPort),
  };
  delete env.RSS_AI_HOST_RUNTIME;
  delete env.CODEX_HOME;
  mark("launch root pnpm dev");
  child = spawn(
    "pnpm",
    [
      "dev",
      "--no-watch",
      "--config",
      JSON.stringify(nativeConfig),
      "--features",
      "native-e2e",
      "--",
      "--",
      "--test-data-dir",
      directory,
    ],
    { cwd: root, env, detached: true, stdio: ["ignore", "pipe", "pipe"] },
  );
  result.devPid = child.pid;
  writeReport();
  child.on("error", (error) => {
    spawnError = error;
  });
  exited = once(child, "exit");
  exited.catch((error) => {
    spawnError = error;
  });
  for (const output of [child.stdout, child.stderr]) {
    let buffer = "";
    output.setEncoding("utf8");
    output.on("data", (chunk) => {
      logs = (logs + chunk).slice(-2 * 1024 * 1024);
      buffer += chunk;
      for (let newline; (newline = buffer.indexOf("\n")) >= 0; ) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        if (line.startsWith("RSS_NATIVE_E2E "))
          receipt = JSON.parse(line.slice(15));
        if (line.startsWith("RSS_AI_HOST_STATUS "))
          hostStatus = JSON.parse(line.slice(19));
      }
    });
  }
  await wait(() => {
    assert.equal(child.exitCode, null, logs.slice(-3000));
    return receipt && hostStatus;
  }, 600000);
  assert.equal(receipt.nonceSha256, sha256(nonce));
  assert.equal(receipt.port, port);
  assert.equal(receipt.dataRootSha256, sha256(directory));
  assert.equal(hostStatus.phase, "ready");
  assert.equal(hostStatus.source, "development_override");
  assert.ok(
    ["native-e2e.keychain", "native-e2e.keychain-db"].some((name) =>
      existsSync(join(directory, name)),
    ),
    "native acceptance must own an isolated OS keychain",
  );
  await wait(() => {
    try {
      return (
        execFileSync(
          "/usr/sbin/lsof",
          ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"],
          { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
        ).trim() === String(receipt.pid)
      );
    } catch {
      return false;
    }
  });
  const artifact = join(root, ".local-ci-runs/ai-host-dev-runtime");
  const manifest = JSON.parse(readFileSync(join(artifact, "manifest.json")));
  assert.equal(manifest.kind, "development");
  assert.equal(manifest.status, "passed");
  assert.equal(manifest.developmentFingerprint, developmentFingerprint(root));
  verifyRuntimeIntegrity(artifact, manifest.runtimeTreeSha256);
  result.runtimeManifestSha256 = sha256(
    readFileSync(join(artifact, "manifest.json")),
  );
  const processField = (pid, field) =>
    execFileSync("/bin/ps", ["-p", String(pid), "-o", field + "="], {
      encoding: "utf8",
    }).trim();
  let ancestor = receipt.pid;
  for (let i = 0; ancestor !== child.pid && ancestor > 1 && i < 16; i++)
    ancestor = Number(processField(ancestor, "ppid"));
  assert.equal(
    ancestor,
    child.pid,
    "driver main must descend from this root pnpm dev",
  );
  const hostPid = () => {
    let children;
    try {
      children = execFileSync("/usr/bin/pgrep", ["-P", String(receipt.pid)], {
        encoding: "utf8",
      })
        .trim()
        .split(/\s+/)
        .map(Number);
    } catch {
      return undefined;
    }
    const hosts = children.filter(
      (pid) => processField(pid, "comm") === join(artifact, "bin/node"),
    );
    assert.ok(hosts.length <= 1, "one owned AI Host process");
    return hosts[0];
  };
  const initialHostPid = hostPid();
  assert.ok(initialHostPid);
  result.owner = {
    mainPid: receipt.pid,
    hostPid: initialHostPid,
    mainBinarySha256: sha256(readFileSync(processField(receipt.pid, "comm"))),
    hostGeneration: hostStatus.generation,
    dataRootSha256: receipt.dataRootSha256,
    driverPort: port,
  };
  for (const authorization of [undefined, `Bearer ${"0".repeat(32)}`]) {
    const response = await fetch(`http://127.0.0.1:${port}/status`, {
      headers: authorization ? { authorization } : {},
    });
    assert.equal(
      response.status,
      401,
      "WebDriver must reject missing/wrong run capability",
    );
  }
  browser = await remote({
    hostname: "127.0.0.1",
    port,
    path: "/",
    logLevel: "silent",
    headers: { Authorization: `Bearer ${nonce}` },
    connectionRetryCount: 0,
    connectionRetryTimeout: 30000,
    capabilities: {
      browserName: "wry",
      "wdio:tauriServiceOptions": { windowLabel: "main" },
    },
  });
  await browser.setTimeout({ implicit: 0, script: 15000 });
  for (const authorization of [undefined, `Bearer ${"0".repeat(32)}`]) {
    const response = await fetch(
      `http://127.0.0.1:${port}/session/${browser.sessionId}/execute/sync`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(authorization ? { authorization } : {}),
        },
        body: JSON.stringify({ script: "return document.title", args: [] }),
      },
    );
    assert.equal(
      response.status,
      401,
      "existing sessions also require the run capability",
    );
  }
  result.checks.push("webdriver-per-request-authentication");
  if (visualOnly) {
    mark("visual fixture in the product main WKWebView");
    await browser.$('[aria-label="测试用户名"]').setValue("Visual Alice");
    await click("进入");
    await wait(() => browser.$('[aria-label="测试用户名"]').isEnabled());
    await navigate("AI 助手");
    await wait(() => browser.$(".composer textarea").isDisplayed());
    await browser.$(".new-conversation").click();
    await browser.setWindowSize(1100, 760);
    await browser.saveScreenshot(
      join(reports, "desktop-native-start-default.png"),
    );
    await navigate("软件中心");
    await wait(() => browser.$(".resource-card").isDisplayed());
    await browser.$('[data-action="resource-details"]').click();
    await click("询问 AI");
    await wait(() => browser.$(".context-panel select").isDisplayed());
    await selectValue(await browser.$(".context-panel select"), "new");
    await click("继续到所选会话");
    await wait(() => browser.$(".resource-context").isDisplayed());
    await browser.saveScreenshot(
      join(reports, "desktop-native-context-default.png"),
    );

    if (!baseline) {
      result.visual = [];
      result.taskFixture =
        "Rust-generated phase/process/effect; synthetic AI association for renderer testing";
      const originalDark =
        script(
          'tell application "System Events" to tell appearance preferences to get dark mode',
        ) === "true";
      const setDark = async (dark) => {
        script(
          `tell application "System Events" to tell appearance preferences to set dark mode to ${dark}`,
        );
        await wait(() =>
          browser.execute(
            (value) =>
              matchMedia("(prefers-color-scheme: dark)").matches === value,
            dark,
          ),
        );
      };
      const capture = async (kind, theme, width) => {
        const metrics = await browser.execute(() => {
          const input = document.querySelector(".composer textarea");
          const r = input.getBoundingClientRect(),
            style = getComputedStyle(input);
          return {
            width: innerWidth,
            height: innerHeight,
            dpr: devicePixelRatio,
            theme: matchMedia("(prefers-color-scheme: dark)").matches
              ? "dark"
              : "light",
            font: style.fontFamily,
            fontSize: style.fontSize,
            inputVisible:
              r.top >= 0 &&
              r.bottom <= innerHeight &&
              r.left >= 0 &&
              r.right <= innerWidth,
            overflow: document.documentElement.scrollWidth > innerWidth,
            assistantCount: document.querySelectorAll(".assistant").length,
            composerCount: document.querySelectorAll(".composer").length,
            mainOpaque: !getComputedStyle(
              document.querySelector(".shell main"),
            ).backgroundColor.includes("rgba"),
            inputOpaque: !style.backgroundColor.includes("rgba"),
          };
        });
        assert.equal(metrics.theme, theme);
        assert.equal(
          metrics.inputVisible,
          true,
          `${kind} ${theme} ${width}: composer reachable`,
        );
        assert.equal(metrics.overflow, false);
        assert.equal(metrics.assistantCount, 1);
        assert.equal(metrics.composerCount, 1);
        assert.equal(metrics.mainOpaque && metrics.inputOpaque, true);
        const screenshot = `desktop-native-${kind}-${theme}-${width}.png`;
        await browser.saveScreenshot(join(reports, screenshot));
        result.visual.push({ kind, screenshot, ...metrics });
      };
      await click("关闭资源上下文 AI");
      await navigate("AI 助手");
      await browser.$(".new-conversation").click();
      await click("移除上下文");
      await browser.execute(() => {
        window.nativeVisualInput = document.querySelector(".composer textarea");
      });
      try {
        for (const dark of [false, true]) {
          const theme = dark ? "dark" : "light";
          await setDark(dark);
          for (const [width, height] of [
            [1100, 760],
            [1440, 960],
            [480, 400],
          ]) {
            mark(`native visual start ${theme} ${width}×${height}`);
            await browser.setWindowSize(width, height);
            await wait(() =>
              browser.execute(
                () =>
                  document.querySelector(".assistant-welcome")?.getClientRects()
                    .length > 0,
              ),
            );
            assert.equal(
              await browser.execute(
                () =>
                  window.nativeVisualInput ===
                  document.querySelector(".composer textarea"),
              ),
              true,
            );
            await capture("start", theme, width);
          }
        }
        await browser.setWindowSize(1100, 760);
        await navigate("AI 助手");
        // This is a real product submission to the explicit FakeHost renderer fixture.
        await prompt("原生视觉验收：解释当前任务记录");
        const sessions = unwrap(
          await visualFixture.host.store.listSessions(visualFixture.caller, {
            limit: 20,
          }),
        );
        const sessionId = sessions.items.find((row) =>
          row.title.startsWith("原生视觉验收"),
        ).namespace.sessionId;
        const command = await visualFixture.command(sessionId);
        await visualFixture.executionDelivery(sessionId, command.commandId);
        await wait(() => browser.$(".execution-activity").isDisplayed());
        unwrap(
          await visualFixture.host.advance(
            visualFixture.caller,
            sessionId,
            command.commandId,
            [{ type: "terminal", outcome: "completed" }],
          ),
        );
        await text("设备状态由独立执行记录呈现。S1 测试投影不代表设备变更。");
        for (const dark of [false, true]) {
          const theme = dark ? "dark" : "light";
          await setDark(dark);
          for (const [width, height] of [
            [1100, 760],
            [1440, 960],
            [480, 400],
          ]) {
            mark(`native visual conversation ${theme} ${width}×${height}`);
            await browser.setWindowSize(width, height);
            await capture("conversation", theme, width);
          }
        }
        for (const [width, height] of [
          [1100, 760],
          [1440, 960],
          [480, 400],
        ]) {
          await browser.setWindowSize(width, height);
          await browser.$('[data-action="execution-details"]').click();
          await wait(() =>
            browser.$(".execution-inspector-content").isDisplayed(),
          );
          await browser.saveScreenshot(
            join(reports, `desktop-native-execution-dark-${width}.png`),
          );
          if (width === 1440) {
            assert.equal(
              await browser.$('aside[aria-label="设备操作详情"]').isDisplayed(),
              true,
            );
            await browser.$('[aria-label="关闭设备操作详情"]').click();
          } else {
            assert.equal(
              await browser
                .$('dialog[aria-label="设备操作详情"]')
                .isDisplayed(),
              true,
            );
            key(53);
          }
          await wait(() =>
            browser.execute(
              () =>
                document.activeElement?.getAttribute("data-action") ===
                "execution-details",
            ),
          );
        }
        result.checks.push(
          "native-task-aside-and-drawer",
          "native-task-focus-return",
          "model-terminal-separate-from-execution",
        );
        for (const dark of [false, true]) {
          const theme = dark ? "dark" : "light";
          await setDark(dark);
          for (const [width, height] of [
            [1100, 760],
            [1440, 960],
            [480, 400],
          ]) {
            mark(`native visual context ${theme} ${width}×${height}`);
            await browser.setWindowSize(width, height);
            await navigate("软件中心");
            await browser.$('[data-action="resource-details"]').click();
            await click("询问 AI");
            await wait(() => browser.$(".context-panel select").isDisplayed());
            await selectValue(await browser.$(".context-panel select"), "new");
            await click("继续到所选会话");
            await wait(() => browser.$(".resource-context").isDisplayed());
            await browser
              .$(".composer textarea")
              .setValue(`保留原生草稿 ${theme} ${width}`);
            await capture("context", theme, width);
            const beforeTab = await browser.execute(
              () =>
                document.activeElement ===
                document.querySelector(".composer textarea"),
            );
            assert.equal(
              beforeTab,
              true,
              "context composer owns focus before Tab",
            );
            key(48);
            await wait(() =>
              browser.execute(
                () =>
                  document.activeElement !==
                    document.querySelector(".composer textarea") &&
                  !!document.activeElement?.closest(".context-panel"),
              ),
            );
            key(48, true);
            await wait(() =>
              browser.execute(
                () =>
                  document.activeElement ===
                  document.querySelector(".composer textarea"),
              ),
            );
            if (width < 1440) {
              key(53);
            } else await click("关闭资源上下文 AI");
            await wait(() =>
              browser.execute(
                () =>
                  document.activeElement?.getAttribute("data-action") ===
                  "ask-ai",
              ),
            );
            await navigate("AI 助手");
            assert.equal(
              await browser.$(".composer textarea").getValue(),
              `保留原生草稿 ${theme} ${width}`,
            );
            assert.equal(
              await browser.execute(
                () =>
                  window.nativeVisualInput ===
                  document.querySelector(".composer textarea"),
              ),
              true,
            );
          }
        }
        result.checks.push(
          "native-six-layout-theme-cases",
          "native-theme-hot-switch",
          "native-context-tab-escape-focus",
          "native-single-composer-draft-preservation",
        );
      } finally {
        await setDark(originalDark);
        result.systemThemeRestored = true;
      }
      const nativePreferences = JSON.parse(
        execFileSync(
          "/usr/bin/osascript",
          [
            "-l",
            "JavaScript",
            "-e",
            'ObjC.import("AppKit"); var w=$.NSWorkspace.sharedWorkspace; JSON.stringify({reduceTransparency:Boolean(w.accessibilityDisplayShouldReduceTransparency),reducedMotion:Boolean(w.accessibilityDisplayShouldReduceMotion),highContrast:Boolean(w.accessibilityDisplayShouldIncreaseContrast)});',
          ],
          { encoding: "utf8", timeout: 10000 },
        ),
      );
      result.appearance = await waitForAppearance(
        () =>
          browser.execute(() => ({
            materialEnabled: document
              .querySelector(".app-root")
              .classList.contains("native-material"),
            reducedMotion: document
              .querySelector(".app-root")
              .classList.contains("reduced-motion"),
            highContrast: document
              .querySelector(".app-root")
              .classList.contains("high-contrast"),
          })),
        nativePreferences,
        wait,
      );
      result.appearance.systemPreferences = nativePreferences;

      // Use an enabled real input source and physical keys; no composition simulation.
      const inputSources = () =>
        JSON.parse(
          execFileSync(
            "/usr/bin/swift",
            [
              "-e",
              'import Carbon; import Foundation; let list = TISCreateInputSourceList(nil, false).takeRetainedValue() as! [TISInputSource]; func id(_ s:TISInputSource)->String { let p=TISGetInputSourceProperty(s,kTISPropertyInputSourceID)!; return Unmanaged<CFString>.fromOpaque(p).takeUnretainedValue() as String }; let current=TISCopyCurrentKeyboardInputSource().takeRetainedValue(); print(String(data:try! JSONSerialization.data(withJSONObject:["current":id(current),"enabled":list.map(id)]),encoding:.utf8)!)',
            ],
            { encoding: "utf8", timeout: 10000 },
          ),
        );
      const selectInputSource = (id) =>
        execFileSync(
          "/usr/bin/swift",
          [
            "-e",
            `import Carbon; import Foundation; let wanted=${JSON.stringify(id)}; let list=TISCreateInputSourceList(nil,false).takeRetainedValue() as! [TISInputSource]; for s in list { let p=TISGetInputSourceProperty(s,kTISPropertyInputSourceID)!; let id=Unmanaged<CFString>.fromOpaque(p).takeUnretainedValue() as String; if id == wanted { if TISSelectInputSource(s) != 0 { exit(1) }; exit(0) } }; exit(1)`,
          ],
          { encoding: "utf8", timeout: 10000 },
        );
      const sources = inputSources();
      if (sources.enabled.includes("com.apple.inputmethod.SCIM.ITABC")) {
        mark("physical native Chinese IME confirmation and explicit send");
        await browser.setWindowSize(1100, 760);
        await navigate("AI 助手");
        await browser.$(".new-conversation").click();
        await browser.$(".composer textarea").setValue("");
        await browser.$(".composer textarea").click();
        const before = await browser.execute(
          async () =>
            (await window.assistantRuntime.listSessions()).items.length,
        );
        await browser.execute(() => {
          window.nativeImeEvents = [];
          const input = document.querySelector(".composer textarea");
          for (const type of ["compositionstart", "compositionend"])
            input.addEventListener(type, (event) =>
              window.nativeImeEvents.push(event.type),
            );
        });
        try {
          selectInputSource("com.apple.inputmethod.SCIM.ITABC");
          for (const code of [45, 34, 4, 0, 31]) key(code);
          await wait(() =>
            browser.execute(() =>
              window.nativeImeEvents.includes("compositionstart"),
            ),
          );
          key(36);
          await wait(() =>
            browser.execute(() =>
              window.nativeImeEvents.includes("compositionend"),
            ),
          );
          const confirmed = await browser.$(".composer textarea").getValue();
          assert.ok(
            confirmed.length > 0,
            "native Return commits the composing text",
          );
          assert.equal(
            await browser.execute(
              async () =>
                (await window.assistantRuntime.listSessions()).items.length,
            ),
            before,
            "IME confirmation must not submit",
          );
          // Apple Pinyin Return commits raw spelling; Space selects its Chinese candidate.
          await browser.$(".composer textarea").setValue("");
          for (const code of [45, 34, 4, 0, 31]) key(code);
          await wait(() =>
            browser.execute(
              () =>
                window.nativeImeEvents.filter(
                  (type) => type === "compositionstart",
                ).length === 2,
            ),
          );
          key(49);
          await wait(() =>
            browser.execute(
              () =>
                window.nativeImeEvents.filter(
                  (type) => type === "compositionend",
                ).length === 2,
            ),
          );
          assert.match(
            await browser.$(".composer textarea").getValue(),
            /[^\x00-\x7f]/,
            "Space selects the native Chinese candidate",
          );
          assert.equal(
            await browser.execute(
              async () =>
                (await window.assistantRuntime.listSessions()).items.length,
            ),
            before,
            "Chinese candidate confirmation must not submit",
          );
          key(36);
          await wait(
            async () =>
              (await browser.execute(
                async () =>
                  (await window.assistantRuntime.listSessions()).items.length,
              )) ===
              before + 1,
          );
          result.ime = {
            status: "passed",
            source: "com.apple.inputmethod.SCIM.ITABC",
            events: await browser.execute(() => window.nativeImeEvents),
            confirmationDidNotSend: true,
            returnConfirmedSpellingWithoutSend: true,
            spaceConfirmedChineseWithoutSend: true,
            explicitReturnSentOnce: true,
          };
          result.messageInput =
            "physical native Pinyin Return and Space confirmation followed by explicit Return submission";
          result.checks.push(
            "native-chinese-IME-confirmation-without-send",
            "native-explicit-Return-send",
          );
        } finally {
          selectInputSource(sources.current);
          result.inputSourceRestored =
            inputSources().current === sources.current;
          assert.equal(result.inputSourceRestored, true);
        }
      } else
        result.ime = {
          status: "unverified",
          reason: "Chinese Pinyin is not enabled on this desktop",
        };
      result.pageZoom = {
        status: "unverified",
        reason:
          "the current product has no native page zoom entry; CSS zoom is not evidence",
      };
      result.displayScaling = {
        status: "observed",
        dpr: [...new Set(result.visual.map((row) => row.dpr))],
        otherScales: "unverified",
      };

      result.windowsAppearance = "unverified; no Windows host";
      await browser.setWindowSize(1100, 760);
      mark("native visual close and reopen main window");
      const closed = native(`
        set mainWindow to first window whose name is "RSS MDM Agent"
        repeat with control in entire contents of mainWindow
          try
            if subrole of control is "AXCloseButton" then
              perform action "AXPress" of control
              return "pressed-main-close-control"
            end if
          end try
        end repeat
        error "main native close control is unavailable"
      `);
      assert.equal(closed, "pressed-main-close-control");
      await wait(async () => (await browser.getWindowHandles()).length === 0);
      menu("显示窗口");
      await wait(async () => (await browser.getWindowHandles()).length === 1);
      await browser.switchToWindow((await browser.getWindowHandles())[0]);
      await wait(() => browser.$(".composer textarea").isDisplayed());
      result.checks.push("native-visual-close-reopen");
    }
    result.checks.push(
      "native-visual-baseline",
      "same-product-App",
      "explicit-resource-context-selection",
    );
    result.baseline = baseline;
    assert.deepEqual(
      sourceEvidence(root),
      result.source,
      "source changed during acceptance",
    );
    result.status = "passed";
  } else {
    mark("account and saved unverified connection");
    await browser.$('[aria-label="测试用户名"]').setValue("Golden Alice");
    await click("进入");
    await wait(() => browser.$('[aria-label="测试用户名"]').isEnabled());
    await navigate("设置");
    await text("个人 AI 连接（0）");
    await (await field("名称")).setValue("Golden Codex");
    await selectValue(await field("认证来源"), "custom_api");
    await (await field("API 地址")).setValue(fixture.url);
    await (await field("模型")).setValue("native-golden-model");
    await browser.$(".connection-form summary").click();
    await selectValue(await field("工具"), "controlled_tools");
    await browser.$('input[type="password"]').setValue(fixture.secret);
    mark("save connection through native credential owner");
    await click("保存配置");
    await wait(
      async () =>
        (await visibleText("Golden Codex · Codex · 未验证")) ||
        (await visibleText("AI Host 不可用，配置尚未保存")),
    );
    assert.equal(await visibleText("Golden Codex · Codex · 未验证"), true);
    result.checks.push("noninteractive-isolated-keychain");
    assert.equal(
      await browser.$('input[type="password"]').isExisting(),
      false,
      "form resets after save",
    );
    assert.equal(fixture.facts.requests, 0, "save must not contact model");
    mark("failed probe then explicit retry");
    await click("测试连接");
    await text("最近测试失败");
    assert.ok(fixture.facts.rejectedAuthentication > 0);
    fixture.accept();
    await click("测试连接");
    await text("Golden Codex · Codex · 可用");
    assert.equal(
      await browser
        .$('[aria-label="将连接 Golden Codex 设为默认"]')
        .isEnabled(),
      false,
      "first saved connection is already the default",
    );
    const sidebar = await browser.execute(() => {
      const el = document.querySelector(".shell aside");
      const style = getComputedStyle(el);
      return {
        width: el.getBoundingClientRect().width,
        padding: style.padding,
        background: style.backgroundColor,
        solidBackground: getComputedStyle(document.querySelector(".shell main"))
          .backgroundColor,
      };
    });
    await click("开始对话");
    assert.equal(await browser.$(".composer textarea").isDisplayed(), true);
    assert.equal(
      await browser.execute(
        () => document.querySelectorAll(".conversation-list li").length,
      ),
      0,
      "entering chat does not create an empty session",
    );
    result.checks.push(
      "saved-unverified",
      "failed-probe-retry",
      "secret-cleared",
      "blank-composer",
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-start-default.png"),
    );
    mark("left primary navigation shares the persistent sidebar theme");
    await browser.setWindowSize(480, 400);
    await wait(() => browser.$('[aria-label="打开主导航"]').isDisplayed());
    await browser.$('[aria-label="打开主导航"]').click();
    const navigation = await browser.execute(() => {
      const el = document.querySelector('dialog[aria-label="主导航"]');
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      return {
        left: rect.left,
        width: rect.width,
        padding: style.padding,
        background: style.backgroundColor,
        footerAtBottom:
          el.querySelector(".navigation-footer").getBoundingClientRect()
            .bottom >
          innerHeight - 40,
      };
    });
    assert.equal(navigation.left, 0);
    assert.equal(navigation.footerAtBottom, true);
    assert.equal(navigation.width, 280);
    assert.equal(navigation.background, sidebar.solidBackground);
    await browser.saveScreenshot(
      join(reports, "desktop-native-navigation.png"),
    );
    key(53);
    await wait(async () => !(await browser.$("dialog[open]").isExisting()));
    await wait(() =>
      browser.execute(
        () =>
          document.activeElement?.getAttribute("aria-label") === "打开主导航",
      ),
    );
    result.checks.push("left-navigation-consistent-style");
    mark("first send, real tool approval and Rust confirmation");
    await browser.setWindowSize(1100, 760);
    await prompt("GOLDEN_INSTALL 安装办公套件");
    await permission(true);
    await text("完成 INSTALL");
    await browser.$('[aria-label="复制代码"]').click();
    await wait(
      async () =>
        (await visibleText("已复制")) ||
        (await visibleText("复制不可用，请选中文本手动复制。")),
    );
    result.clipboard = (await visibleText("已复制"))
      ? "writeText"
      : "explicit-selection-fallback";
    result.checks.push("native-copy-or-explicit-fallback");

    await text("等待用户确认本次动作");
    assert.equal(
      task().snapshot.attempts,
      0,
      "AI permission cannot replace Rust action confirmation",
    );
    await click("查看设备操作");
    await text("等待用户确认本次动作");
    // Native Escape must close dialog and return focus to its trigger.
    key(53);
    await wait(async () => !(await browser.$("dialog[open]").isExisting()));
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      "查看设备操作",
    );
    await prompt("GOLDEN_CANCEL_REJECT 拒绝取消");
    await permission(false);
    await text("完成 CANCEL_REJECT");
    assert.equal(task().snapshot.cancelRequested, false);
    await prompt("GOLDEN_DENY 拒绝新的操作");
    await permission(false);
    await text("完成 DENY");
    assert.equal(
      dbRead(
        "execution.sqlite",
        (db) => db.prepare("SELECT count(*) n FROM executions").get().n,
      ),
      1,
    );
    await click("查看设备操作");
    await click("前往任务确认动作");
    await wait(async () => !(await browser.$("dialog[open]").isExisting()));
    await click("刷新任务");
    await browser.$(".task-list .task-row").click();
    await click("确认并执行");
    await wait(() => task()?.snapshot.attempts === 1);
    await navigate("AI 助手");
    await prompt("GOLDEN_CANCEL_ALLOW 允许取消请求");
    await permission(true);
    await text("完成 CANCEL_ALLOW");
    await wait(() => task()?.snapshot.cancelRequested === true);
    await prompt("GOLDEN_READ 读取状态");
    await text("完成 READ");
    assert.equal(
      await browser.$('[aria-label="AI 工具权限请求"]').isExisting(),
      false,
    );
    result.checks.push(
      "first-send-single-session",
      "execute-allow-reject",
      "cancel-allow-reject",
      "reads-without-approval",
      "rust-confirmation-separate",
      "trusted-execution-card",
    );
    mark("manual history position survives a new reply");
    await browser.$(".assistant-timeline article:first-child").scrollIntoView();
    await wait(
      async () =>
        await browser
          .$('//button[normalize-space(.)="回到最新消息"]')
          .isDisplayed(),
    );
    const scrollTop = await browser.execute(
      () => document.querySelector(".assistant-timeline").scrollTop,
    );
    await prompt("GOLDEN_HELLO 保留阅读位置");
    await text("完成 HELLO");
    assert.ok(
      Math.abs(
        (await browser.execute(
          () => document.querySelector(".assistant-timeline").scrollTop,
        )) - scrollTop,
      ) < 2,
    );
    await click("回到最新消息");
    result.checks.push("manual-scroll-preserved", "jump-to-latest");
    mark("narrow layout, native focus and scrolling");
    await browser.setWindowSize(480, 400);
    await wait(() => browser.$('[aria-label="打开主导航"]').isDisplayed());
    await browser.$('[aria-label="打开主导航"]').click();
    assert.equal(
      await browser.execute(() => {
        const r = document
          .querySelector('dialog[aria-label="主导航"]')
          .getBoundingClientRect();
        return r.left === 0 && r.right <= innerWidth;
      }),
      true,
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-navigation-narrow.png"),
    );
    key(53);
    await wait(async () => !(await browser.$("dialog[open]").isExisting()));
    await browser.$('[aria-label="打开主导航"]').click();
    assert.equal(
      await browser.execute(
        () =>
          document
            .querySelector('dialog[aria-label="主导航"]')
            .getBoundingClientRect().left,
      ),
      0,
    );
    for (let i = 0; i < 14; i++) {
      const previous = await browser.execute(
        () => document.activeElement?.outerHTML,
      );
      key(48, i >= 7);
      await wait(() =>
        browser.execute(
          (previous) =>
            Boolean(document.activeElement?.closest("dialog[open]")) &&
            document.activeElement?.outerHTML !== previous,
          previous,
        ),
      );
    }
    key(53);
    await wait(async () => !(await browser.$("dialog[open]").isExisting()));
    assert.equal(
      await browser.execute(() =>
        document.activeElement?.getAttribute("aria-label"),
      ),
      "打开主导航",
    );
    await browser.$(".composer textarea").setValue("保留键盘验收草稿");
    await browser.$(".composer textarea").click();
    assert.equal(
      await browser.$('.composer button[type="submit"]').isEnabled(),
      true,
      "draft enables native send control",
    );
    // ref: wry@0.55.1 src/wkwebview/mod.rs enables tabFocusesLinks.
    // System Events enqueues key input; wait for WKWebView to process it.
    key(48);
    await wait(() =>
      browser.execute(
        () =>
          document.activeElement ===
          document.querySelector('.composer button[type="submit"]'),
      ),
    );
    key(48, true);
    await wait(() =>
      browser.execute(
        () =>
          document.activeElement ===
          document.querySelector(".composer textarea"),
      ),
    );
    await browser.$(".composer textarea").setValue("");
    assert.equal(
      await browser.execute(
        () => document.querySelectorAll('[role="log"]').length,
      ),
      1,
    );
    assert.equal(
      await browser.execute(() => {
        const r = document
          .querySelector(".composer textarea")
          .getBoundingClientRect();
        return r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth;
      }),
      true,
    );
    assert.equal(
      await browser.execute(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      true,
      "native page has no horizontal overflow",
    );
    await browser.saveScreenshot(join(reports, "desktop-native-narrow.png"));
    await browser.setWindowSize(1100, 760);
    await wait(() =>
      browser.$(".navigation-panel .conversation-list").isDisplayed(),
    );
    result.checks.push(
      "native-tab-shift-tab-escape",
      "focus-restoration",
      "narrow-composer",
      "single-live-log",
    );
    assert.equal(
      await browser.execute(
        () => document.querySelectorAll(".conversation-list li").length,
      ),
      1,
      "all first-chat operations share one session",
    );
    mark("resource context in the same native conversation at three sizes");
    const proposalsBeforeContext = fixture.facts.proposals.length;
    for (const [width, height] of [
      [1100, 760],
      [480, 400],
      [1440, 960],
    ]) {
      mark(`resource context ${width}×${height}`);
      await browser.setWindowSize(width, height);
      await wait(() =>
        browser.execute(
          (expected) => innerWidth < 900 === expected,
          width < 900,
        ),
      );
      await navigate("软件中心");
      await wait(() =>
        browser.$('[data-action="resource-details"]').isDisplayed(),
      );
      await browser.$('[data-action="resource-details"]').click();
      await click("询问 AI");
      await wait(() => browser.$(".context-panel select").isDisplayed());
      assert.equal(
        await browser.$('[data-action="choose-conversation"]').isEnabled(),
        false,
      );
      const session = await browser.execute(
        () =>
          [...document.querySelector(".context-panel select").options].find(
            (option) => option.value && option.value !== "new",
          ).value,
      );
      await selectValue(await browser.$(".context-panel select"), session);
      await click("继续到所选会话");
      await wait(() => browser.$(".resource-context").isDisplayed());
      if (width < 1440) {
        await browser.$(".connection-trigger").click();
        await wait(() =>
          browser.$('.context-panel [aria-label="AI 连接选择"]').isDisplayed(),
        );
        key(53);
        await wait(
          async () =>
            !(await browser.$('[aria-label="AI 连接选择"]').isDisplayed()),
        );
        await browser.$('[aria-label="更多"]').click();
        await wait(() =>
          browser.$('.context-panel [role="menuitem"]').isDisplayed(),
        );
        await browser.$('.context-panel [role="menuitem"]').click();
        await wait(() =>
          browser.$('dialog[aria-label="会话详情与诊断"]').isDisplayed(),
        );
        await wait(() =>
          browser.execute(
            () =>
              document.activeElement
                .closest("dialog")
                ?.getAttribute("aria-label") === "会话详情与诊断",
          ),
        );
        key(48);
        await wait(() =>
          browser.execute(
            () =>
              document.activeElement
                .closest("dialog")
                ?.getAttribute("aria-label") === "会话详情与诊断",
          ),
        );
        key(53);
        await wait(
          async () =>
            !(await browser
              .$('dialog[aria-label="会话详情与诊断"]')
              .isExisting()),
        );
      }
      assert.equal(
        await browser.execute(
          () => document.querySelectorAll(".assistant").length,
        ),
        1,
      );
      await browser
        .$(".composer textarea")
        .setValue("GOLDEN_CONTEXT 解释这个软件的版本与限制");
      assert.equal(
        await browser.execute(() => {
          const r = document
            .querySelector(".composer textarea")
            .getBoundingClientRect();
          return (
            r.bottom <= innerHeight &&
            r.left >= 0 &&
            r.right <= innerWidth &&
            document.documentElement.scrollWidth <= innerWidth
          );
        }),
        true,
      );
      if (width === 1100)
        await browser.saveScreenshot(
          join(reports, "desktop-native-context-default.png"),
        );
      if (width === 480)
        await browser.saveScreenshot(
          join(reports, "desktop-native-context-narrow.png"),
        );
      if (width === 1440)
        await browser.saveScreenshot(
          join(reports, "desktop-native-context-wide.png"),
        );
      await click("关闭资源上下文 AI");
      await wait(() =>
        browser.execute(
          () =>
            document.activeElement?.getAttribute("data-action") === "ask-ai",
        ),
      );
      await navigate("AI 助手");
      assert.equal(
        await browser.$(".composer textarea").getValue(),
        "GOLDEN_CONTEXT 解释这个软件的版本与限制",
      );
    }
    await prompt("GOLDEN_CONTEXT 解释这个软件的版本与限制");
    await text("完成 CONTEXT");
    assert.equal(fixture.facts.contexts.length, 1);
    assert.equal(
      fixture.facts.proposals.length,
      proposalsBeforeContext,
      "context inquiry does not execute tools",
    );
    result.checks.push(
      "resource-context-explicit-selection",
      "resource-context-same-session",
      "resource-context-provider-payload",
      "resource-context-three-sizes",
    );
    const readAppearance = () =>
      browser.execute(() => ({
        materialEnabled: document
          .querySelector(".app-root")
          .classList.contains("native-material"),
        reducedMotion: document
          .querySelector(".app-root")
          .classList.contains("reduced-motion"),
        highContrast: document
          .querySelector(".app-root")
          .classList.contains("high-contrast"),
        bodyBackground: getComputedStyle(document.querySelector(".shell main"))
          .backgroundColor,
      }));
    const systemAppearance = JSON.parse(
      execFileSync(
        "/usr/bin/osascript",
        [
          "-l",
          "JavaScript",
          "-e",
          'ObjC.import("AppKit"); var w=$.NSWorkspace.sharedWorkspace; JSON.stringify({reduceTransparency:Boolean(w.accessibilityDisplayShouldReduceTransparency),reducedMotion:Boolean(w.accessibilityDisplayShouldReduceMotion),highContrast:Boolean(w.accessibilityDisplayShouldIncreaseContrast)});',
        ],
        { encoding: "utf8", timeout: 10000 },
      ),
    );
    const expectedMaterial =
      !systemAppearance.reduceTransparency &&
      !systemAppearance.reducedMotion &&
      !systemAppearance.highContrast;
    result.appearance = await waitForAppearance(
      readAppearance,
      systemAppearance,
      wait,
    );
    assert.ok(
      !result.appearance.bodyBackground.includes("rgba"),
      "body remains opaque",
    );
    assert.equal(
      result.appearance.materialEnabled,
      expectedMaterial,
      "material activation matches actual AppKit preferences",
    );
    assert.equal(
      result.appearance.reducedMotion,
      systemAppearance.reducedMotion,
    );
    assert.equal(result.appearance.highContrast, systemAppearance.highContrast);
    result.appearance.systemPreferences = systemAppearance;
    result.appearance.currentTheme = await browser.execute(() =>
      matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
    );
    result.appearance.unverified = [
      "light/dark theme hot switching",
      "reduce transparency/motion/high contrast hot switching",
      "physical titlebar drag",
    ];
    result.checks.push(
      "native-material-current-system-policy",
      "native-body-opaque",
      "context-popover-menu-modal-host",
    );
    result.windowsAppearance = "unverified; no Windows host in this run";
    await browser.setWindowSize(1100, 760);
    mark("close/reopen main window and restore history");
    native(
      'perform action "AXPress" of (first button of window 1 whose subrole is "AXCloseButton")',
    );
    await wait(async () => (await browser.getWindowHandles()).length === 0);
    menu("显示窗口");
    await wait(async () => (await browser.getWindowHandles()).length === 1);
    await browser.switchToWindow((await browser.getWindowHandles())[0]);
    mark("restore conversation in reopened window");
    await wait(() =>
      browser.execute(
        (expected) =>
          !!document.querySelector(".app-root") &&
          document
            .querySelector(".app-root")
            .classList.contains("native-material") === expected,
        expectedMaterial,
      ),
    );
    result.checks.push("native-close-control-material-rebind");
    await wait(() => browser.$(".composer textarea").isDisplayed());
    await wait(async () => !(await visibleText("正在读取或切换账户…")));
    await navigate("AI 助手");
    await text("GOLDEN_INSTALL 安装办公套件");
    mark("restart Host through settings");
    await navigate("设置");
    await click("重启 AI Host");
    await click("确认重启");
    await wait(() => {
      const pid = hostPid();
      return pid && pid !== initialHostPid;
    });
    result.owner.restartedHostPid = hostPid();
    mark("reuse native credential after Host restart");
    await text("AI Host：已就绪");
    await wait(
      async () =>
        await browser
          .$('//button[normalize-space(.)="重启 AI Host"]')
          .isEnabled(),
    );
    await click("测试连接");
    await text("Golden Codex · Codex · 可用");
    mark("restore history and create a new provider session after restart");
    await navigate("AI 助手");
    await text("GOLDEN_INSTALL 安装办公套件");
    await browser.$(".conversation-list .new-conversation").click();
    await prompt("GOLDEN_HELLO 重启后新对话");
    await text("完成 HELLO");
    result.checks.push(
      "window-reopen-history",
      "host-restart-credential-reuse",
      "new-provider-session-after-restart",
    );
    mark("caller isolation and credential deletion");
    await navigate("设置");
    await browser.$('[aria-label="测试用户名"]').setValue("Golden Bob");
    await click("进入");
    await wait(() => browser.$('[aria-label="测试用户名"]').isEnabled());
    await navigate("设置");
    await text("个人 AI 连接（0）");
    await click("稍后配置，前往 AI");
    assert.equal(await visibleText("GOLDEN_INSTALL 安装办公套件"), false);
    await navigate("设置");
    await browser.$('[aria-label="测试用户名"]').setValue("Golden Alice");
    await click("进入");
    await wait(() => browser.$('[aria-label="测试用户名"]').isEnabled());
    await navigate("设置");
    await text("个人 AI 连接（1）");
    await click("删除");
    await click("确认删除");
    await text("个人 AI 连接（0）");
    assert.equal(
      (await browser.getPageSource()).includes(fixture.secret),
      false,
    );
    assert.equal(logs.includes(fixture.secret), false);
    result.checks.push(
      "caller-isolation",
      "credential-deletion",
      "secret-redaction",
    );
    mark("graceful application menu quit");
    await browser.deleteSession();
    browser = undefined;
    menu("退出 RSS MDM Agent");
    await wait(
      () => child.exitCode !== null || child.signalCode !== null,
      15000,
    );
    result.exit = { code: child.exitCode, signal: child.signalCode };
    assert.equal(child.exitCode, 0);
    await wait(() => {
      try {
        process.kill(receipt.pid, 0);
        return false;
      } catch (error) {
        return error.code === "ESRCH";
      }
    });
    for (const pid of [initialHostPid, result.owner.restartedHostPid]) {
      await wait(() => {
        try {
          process.kill(pid, 0);
          return false;
        } catch (error) {
          return error.code === "ESRCH";
        }
      });
    }
    // AI SQLite has a single exclusive owner; inspect persisted facts only after quit.
    assert.equal(
      dbRead(
        "ai.sqlite",
        (db) =>
          db
            .prepare(
              "SELECT count(*) n FROM connections WHERE encrypted_secret IS NOT NULL",
            )
            .get().n,
      ),
      0,
    );
    const sessions = dbRead("ai.sqlite", (db) =>
      db
        .prepare("SELECT json FROM sessions")
        .all()
        .map((row) => JSON.parse(row.json)),
    );
    assert.equal(sessions.length, 2);
    assert.equal(
      new Set(sessions.map((session) => session.namespace.principalId)).size,
      1,
    );
    assert.equal(sessions[0].stages[0].binding.providerVersion, "0.155.0");
    assert.equal(task().snapshot.attempts, 1);
    result.facts = {
      sessions: sessions.length,
      attempts: task().snapshot.attempts,
      model: fixture.facts,
    };
    result.checks.push("native-menu-quit", "host-processes-reaped");
    assert.deepEqual(
      sourceEvidence(root),
      result.source,
      "source changed during acceptance",
    );
    assert.equal(
      sha256(readFileSync(join(artifact, "manifest.json"))),
      result.runtimeManifestSha256,
    );
    verifyRuntimeIntegrity(artifact, manifest.runtimeTreeSha256);
    result.status = "passed";
  }
} catch (error) {
  result.status = cancelled ? "cancelled" : "failed";
  const detail = String(error)
    .replaceAll(nonce, "[redacted-capability]")
    .replaceAll(fixture?.secret ?? "<none>", "[redacted]")
    .replaceAll(directory ?? "<none>", "[isolated-data]")
    .replaceAll(root, "[worktree]")
    .slice(0, 2048);
  result.failure = { stage, code: "native_golden_path_failed", detail };
  result.modelFacts = fixture?.facts;
  if (browser) {
    try {
      result.transportFailure = await browser.execute(() => {
        const c =
          document.querySelector(".assistant")?.__vueParentComponent?.props
            ?.controller;
        const reason = c?.runtime?.value?.connection?.signal?.reason;
        return {
          visible:
            document.querySelector(".assistant")?.__vueParentComponent?.props
              .visible,
          display: document.querySelector(".assistant")?.style.display,
          state: c?.state?.connection ?? "unknown",
          code: typeof reason?.code === "string" ? reason.code : "unknown",
          name: typeof reason?.name === "string" ? reason.name : "unknown",
        };
      });
    } catch {
      result.transportFailure = { state: "unavailable" };
    }
  }
  console.error(`Native acceptance failed at ${stage}: ${detail}`);
  if (browser && !cancelled) {
    try {
      await browser.saveScreenshot(join(reports, "desktop-native-failed.png"));
      console.error((await browser.$("body").getText()).slice(-3500));
    } catch {}
  }
  process.exitCode = 1;
} finally {
  if (browser && !cancelled)
    await Promise.race([browser.deleteSession().catch(() => {}), delay(5000)]);
  if (rootAlive()) {
    if (receipt?.pid) {
      try {
        menu("退出 RSS MDM Agent");
      } catch {}
    }
    await Promise.race([exited.catch(() => {}), delay(7000)]);
    if (rootAlive()) {
      signalRoot("SIGTERM");
      await Promise.race([exited.catch(() => {}), delay(7000)]);
    }
    if (rootAlive()) {
      signalRoot("SIGKILL");
      await Promise.race([exited.catch(() => {}), delay(1000)]);
      result.cleanup = "forced-wrapper-stop";
    }
  }
  await Promise.race([fixture?.close(), delay(2000)]);
  await Promise.race([visualFixture?.close(), delay(2000)]);
  const owned = [
    receipt?.pid,
    result.owner?.hostPid,
    result.owner?.restartedHostPid,
  ].filter(Boolean);
  const alive = () =>
    owned.filter((pid) => {
      try {
        process.kill(pid, 0);
        return true;
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
        return false;
      }
    });
  const cleanupDeadline = Date.now() + 7000;
  while ((alive().length || rootAlive()) && Date.now() < cleanupDeadline)
    await delay(100);
  const cleanupComplete = alive().length === 0 && !rootAlive();
  if (!cleanupComplete) {
    result.cleanup = "owned-processes-still-present";
    result.status = cancelled ? "cancelled" : "failed";
    process.exitCode = 1;
  }
  let keychainClean = cleanupComplete;
  if (directory && cleanupComplete) {
    try {
      const path = ["native-e2e.keychain", "native-e2e.keychain-db"]
        .map((name) => join(directory, name))
        .find(existsSync);
      if (path)
        execFileSync("/usr/bin/security", ["delete-keychain", path], {
          timeout: 10000,
          stdio: "pipe",
        });
      assert.deepEqual(systemKeychains(), keychainState);
      result.keychainCleanup =
        "removed; default keychain and search list unchanged";
    } catch {
      keychainClean = false;
      result.cleanup = "isolated-keychain-cleanup-unconfirmed";
      result.status = cancelled ? "cancelled" : "failed";
      process.exitCode = 1;
    }
  }
  writeReport();
  writeFileSync(
    join(reports, "desktop-native.log"),
    logs
      .replaceAll(nonce, "[redacted-capability]")
      .replaceAll(fixture?.secret ?? "<none>", "[redacted]")
      .replaceAll(directory ?? "<none>", "[isolated-data]"),
  );
  if (directory && cleanupComplete && keychainClean)
    rmSync(directory, { recursive: true, force: true });
}
