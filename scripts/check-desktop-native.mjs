// Daily macOS golden path: root pnpm dev -> production main -> real WKWebView.
// ref: webdriverio/desktop-mobile packages/tauri-plugin-webdriver@1.4.0;
// webdriverio/webdriverio packages/webdriverio/src/index.ts@v9.32.0.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import {
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
import { sourceEvidence, sha256 } from "./native-evidence.mjs";
import { developmentFingerprint } from "./desktop-dev-runtime.mjs";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const reports = join(root, ".local-ci-runs");
mkdirSync(reports, { recursive: true });
const result = {
  status: "running",
  mode: "pnpm-dev-main",
  modelFixture: true,
  credentials: "synthetic-loopback-only",
  provider: "codex-0.155.0",
  executor: "S1 deterministic test runner",
  checks: [],
};
for (const name of [
  "desktop-native-narrow.png",
  "desktop-native-failed.png",
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
  spawnError;
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
      `${scope}//button[normalize-space(.)=${JSON.stringify(name)}]`,
    ))
      if ((await candidate.isDisplayed()) && (await candidate.isEnabled()))
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
  if (await browser.$('[aria-label="打开主导航"]').isDisplayed())
    await browser.$('[aria-label="打开主导航"]').click();
  await click(name);
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
    "composer must own keyboard focus before native Enter",
  );
  key(36);
};
const selectNext = async (element, value) => {
  await element.click();
  key(125);
  key(125);
  key(36);
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
  const nonce = randomBytes(16).toString("hex");
  const env = {
    ...process.env,
    RSS_NATIVE_E2E_NONCE: nonce,
    RSS_NATIVE_E2E_PORT: String(port),
  };
  delete env.RSS_AI_HOST_RUNTIME;
  delete env.CODEX_HOME;
  mark("launch root pnpm dev");
  child = spawn(
    "pnpm",
    [
      "dev",
      "--no-watch",
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
  assert.equal(receipt.nonce, nonce);
  assert.equal(receipt.port, port);
  assert.equal(receipt.dataRootSha256, sha256(directory));
  assert.equal(hostStatus.phase, "ready");
  assert.equal(hostStatus.source, "development_override");
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
  browser = await remote({
    hostname: "127.0.0.1",
    port,
    path: "/",
    logLevel: "silent",
    connectionRetryCount: 0,
    connectionRetryTimeout: 30000,
    capabilities: {
      browserName: "wry",
      "wdio:tauriServiceOptions": { windowLabel: "main" },
    },
  });
  await browser.setTimeout({ implicit: 0, script: 15000 });
  mark("account and saved unverified connection");
  await browser.$('[aria-label="测试用户名"]').setValue("Golden Alice");
  await click("进入");
  await text("个人 AI 连接（0）");
  await (await field("名称")).setValue("Golden Codex");
  await selectNext(await field("认证来源"), "custom_api");
  await (await field("API 地址")).setValue(fixture.url);
  await (await field("模型")).setValue("native-golden-model");
  await browser.$(".connection-form summary").click();
  await selectNext(await field("工具"), "controlled_tools");
  await browser.$('input[type="password"]').setValue(fixture.secret);
  mark("save connection through native credential owner");
  await click("保存配置");
  const keychainPending = () => {
    try {
      return script(
        'tell application "System Events"\n if not (exists process "SecurityAgent") then return ""\n tell process "SecurityAgent"\n if (count of windows) is 0 then return ""\n return value of every static text of window 1\n end tell\nend tell',
      ).includes("RSS MDM Agent");
    } catch {
      return false;
    }
  };
  await wait(
    async () =>
      (await visibleText("Golden Codex · Codex · 未验证")) || keychainPending(),
  );
  if (keychainPending()) {
    mark("waiting for human macOS Keychain authorization");
    await wait(() => !keychainPending(), 120000);
    await wait(
      async () =>
        (await visibleText("Golden Codex · Codex · 未验证")) ||
        (await visibleText("AI Host 不可用，配置尚未保存")),
    );
    if (!(await visibleText("Golden Codex · Codex · 未验证"))) {
      await browser.$('input[type="password"]').setValue(fixture.secret);
      await click("保存配置");
    }
  }
  await text("Golden Codex · Codex · 未验证");
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
    await browser.$('[aria-label="将连接 Golden Codex 设为默认"]').isEnabled(),
    false,
    "first saved connection is already the default",
  );
  await click("开始对话");
  assert.equal(await browser.$(".composer textarea").isDisplayed(), true);
  assert.equal(
    await browser.execute(
      () => document.querySelectorAll(".assistant-sessions li").length,
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
  mark("first send, real tool approval and Rust confirmation");
  await prompt("GOLDEN_INSTALL 安装办公套件");
  await permission(true);
  await text("完成 INSTALL");
  await text("需要确认具体动作");
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
  await browser.setWindowSize(600, 680);
  await browser.$('[aria-label="打开最近对话"]').click();
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
    "打开最近对话",
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
  key(48, true);
  await wait(() =>
    browser.execute(
      () =>
        document.activeElement ===
        document.querySelector('.composer button[type="submit"]'),
    ),
  );
  key(48);
  await wait(() =>
    browser.execute(
      () =>
        document.activeElement === document.querySelector(".composer textarea"),
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
  result.checks.push(
    "native-tab-shift-tab-escape",
    "focus-restoration",
    "narrow-composer",
    "single-live-log",
  );
  assert.equal(
    await browser.execute(
      () => document.querySelectorAll(".assistant-sessions li").length,
    ),
    1,
    "all first-chat operations share one session",
  );
  mark("close/reopen main window and restore history");
  await browser.closeWindow();
  await wait(async () => (await browser.getWindowHandles()).length === 0);
  menu("显示窗口");
  await wait(async () => (await browser.getWindowHandles()).length === 1);
  await browser.switchToWindow((await browser.getWindowHandles())[0]);
  await navigate("AI 助手");
  await text("GOLDEN_INSTALL 安装办公套件");
  await navigate("设置");
  await click("重启 AI Host");
  await click("确认重启");
  await wait(() => {
    const pid = hostPid();
    return pid && pid !== initialHostPid;
  });
  result.owner.restartedHostPid = hostPid();
  await text("AI Host：已就绪");
  await click("测试连接");
  await text("Golden Codex · Codex · 可用");
  await navigate("AI 助手");
  await text("GOLDEN_INSTALL 安装办公套件");
  await browser.$(".assistant-sessions .new-conversation").click();
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
  await text("个人 AI 连接（0）");
  await click("稍后配置，前往 AI");
  assert.equal(await visibleText("GOLDEN_INSTALL 安装办公套件"), false);
  await navigate("设置");
  await browser.$('[aria-label="测试用户名"]').setValue("Golden Alice");
  await click("进入");
  await text("个人 AI 连接（1）");
  await click("删除");
  await click("确认删除");
  await text("个人 AI 连接（0）");
  assert.equal((await browser.getPageSource()).includes(fixture.secret), false);
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
  await wait(() => child.exitCode !== null || child.signalCode !== null, 15000);
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
} catch (error) {
  result.status = cancelled ? "cancelled" : "failed";
  const detail = String(error)
    .replaceAll(fixture?.secret ?? "<none>", "[redacted]")
    .replaceAll(directory ?? "<none>", "[isolated-data]")
    .replaceAll(root, "[worktree]")
    .slice(0, 2048);
  result.failure = { stage, code: "native_golden_path_failed", detail };
  result.modelFacts = fixture?.facts;
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
  writeReport();
  writeFileSync(
    join(reports, "desktop-native.log"),
    logs
      .replaceAll(fixture?.secret ?? "<none>", "[redacted]")
      .replaceAll(directory ?? "<none>", "[isolated-data]"),
  );
  if (directory && cleanupComplete)
    rmSync(directory, { recursive: true, force: true });
}
