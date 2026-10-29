// Daily macOS golden path: root pnpm dev -> production main -> real WKWebView.
// ref: webdriverio/desktop-mobile packages/tauri-plugin-webdriver@1.4.0;
// webdriverio/webdriverio packages/webdriverio/src/index.ts@v9.32.0.
import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
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
import { cargoTargetDir } from "./cargo-target.mjs";
import {
  runPreparation,
  reapOwnedProcessGroup,
  processField,
  processChildren,
  runOwnedScopeProbe,
} from "./desktop-dev-process.mjs";
import { randomBytes } from "node:crypto";
import { createServer } from "node:net";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { remote } from "webdriverio";
import { startModelFixture } from "../tests/desktop/model-fixture.mjs";
import {
  sourceEvidence,
  toolchainEvidence,
  sha256,
  waitForAppearance,
  nativeReplyComplete,
  sampledNativeTask,
} from "./native-evidence.mjs";
import {
  verifyCandidate,
  verifyNativeRuntime,
  nativeProductModules,
} from "./native-candidate.mjs";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";

import { nativeOptions } from "./native-options.mjs";
const {
  visualOnly,
  baseline,
  controlledService,
  security,
  candidatePath,
  authorizationPasswordFile,
} = nativeOptions(process.argv.slice(2));
let serviceWorker,
  serviceReady,
  serviceFinishing = false,
  browserClosing,
  serviceOutput = "",
  serviceSequence = 0;
const servicePending = new Map();
function serviceCall(method, input = {}) {
  return new Promise((resolve, reject) => {
    const id = ++serviceSequence;
    const timer = setTimeout(
      () => {
        servicePending.delete(id);
        reject(new Error(`service ${method} deadline exceeded`));
      },
      method === "finish"
        ? 240000
        : ["completion", "security"].includes(method)
          ? 90000
          : 15000,
    );
    servicePending.set(id, {
      resolve(value) {
        clearTimeout(timer);
        resolve(value);
      },
      reject(error) {
        clearTimeout(timer);
        reject(error);
      },
    });
    serviceWorker.stdin.write(JSON.stringify({ id, method, ...input }) + "\n");
  });
}
let awake;
const root = fileURLToPath(new URL("../", import.meta.url));
const reports = join(root, ".local-ci-runs");
const nonce = randomBytes(16).toString("hex");
mkdirSync(reports, { recursive: true });
const result = {
  status: "running",
  mode: "pnpm-dev-main",
  modelFixture: !visualOnly,
  credentials:
    "synthetic-loopback-only; not exercised until preflight succeeds",
  provider: visualOnly
    ? "scripted fixture Host; not yet exercised"
    : "codex-0.155.0; not yet exercised",
  executor: "not exercised; preflight pending",
  keychain: visualOnly
    ? "not used"
    : "isolated keychain planned; not yet created",
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
let scopeRuntime, productModules, scopeAbsentWithin;
const ownedWorkerScopes = new Map();
const processIdentity = processField;
const rememberScopes = (rows, host) => {
  for (const row of rows) {
    if (row.scope?.kind !== "processGroup") continue;
    const scope = row.scope;
    if (ownedWorkerScopes.has(scope.root)) continue;
    let anchor;
    if (
      scopeRuntime &&
      processIdentity(scope.root, "comm") === scopeRuntime.launcher &&
      Number(processIdentity(scope.root, "ppid")) === host
    ) {
      const children = processChildren(scope.root);
      const pid = children.find(
        (child) => processIdentity(child, "comm") === scopeRuntime.node,
      );
      if (pid && Number(processIdentity(pid, "pgid")) === scope.root)
        anchor = { pid, start: processIdentity(pid, "lstart") };
    }
    ownedWorkerScopes.set(scope.root, {
      scope,
      anchor,
      launchId: row.launchId,
    });
  }
};
const terminateObservedScope = (entry) => {
  if (
    !entry.anchor ||
    processIdentity(entry.anchor.pid, "lstart") !== entry.anchor.start ||
    Number(processIdentity(entry.anchor.pid, "pgid")) !== entry.scope.root
  )
    return false;
  try {
    process.kill(-entry.scope.root, "SIGKILL");
    return true;
  } catch (error) {
    if (error.code === "ESRCH") return true;
    throw error;
  }
};
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
const faultAbort = new AbortController();
let cancelled = false;
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => {
    cancelled = true;
    faultAbort.abort();
    spawnError = new Error("native acceptance cancelled");
    // The dev wrapper owns Tauri/Vite/main and their bounded process-group shutdown.
    signalRoot("SIGTERM");
    // The coordinator retains the administrator channel until finally reaps our clients.
  });
const mark = (value) => {
  stage = value;
  result.stage = value;
  writeReport();
  console.log(`[native] ${value}`);
};
const serviceFailed = (error) => {
  spawnError ??= error;
  faultAbort.abort(error);
  for (const pending of servicePending.values()) pending.reject(spawnError);
  servicePending.clear();
  // Closing the owned session interrupts an in-flight WebDriver command as well as wait().
  browserClosing ??= browser?.deleteSession().catch(() => {});
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
const native = (source, requireForeground = true) =>
  script(
    `tell application "System Events"\n tell (first application process whose unix id is ${receipt.pid})\n set frontmost to true\n if (count of windows) > 0 then perform action "AXRaise" of window 1\n repeat 40 times\n if frontmost then exit repeat\n set frontmost to true\n delay 0.1\n end repeat\n delay 0.2\n ${requireForeground ? 'if not frontmost then error "Owned application foreground unavailable"' : ""}\n ${source}\n end tell\nend tell`,
  );
const key = (code, shift = false, command = false) => {
  const foreground = native("get frontmost", false);
  if (foreground !== "true")
    result.foregroundFailure = script(
      'tell application "System Events" to get {name, unix id} of (first application process whose frontmost is true)',
    );
  assert.equal(
    foreground,
    "true",
    "native application must own foreground keyboard focus",
  );
  native(
    `key code ${code}${command ? " using command down" : shift ? " using shift down" : ""}`,
  );
};
const menu = (label) =>
  native(
    `repeat with candidate in menu bar items of menu bar 1
      if exists menu item "${label}" of menu 1 of candidate then
        click menu item "${label}" of menu 1 of candidate
        return
      end if
    end repeat
    error "Owned application menu item missing: ${label}"`,
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
const field = (label) => browser.$(`[aria-label="${label}"]`);
const closeNavigationDialog = async () => {
  // Ant Design portals stay mounted while hidden; only close a visible dialog.
  if (
    await browser.execute(() =>
      [...document.querySelectorAll('[role="dialog"]')].some(
        (node) => node.getClientRects().length > 0,
      ),
    )
  )
    key(53);
};
const expandNavigation = async () => {
  for (let count = 0; count < 4; count++) {
    if (
      (await browser
        .$(".rss-desktop-shell")
        .getAttribute("data-layout-step")) === "2"
    )
      return;
    await click("切换侧栏布局");
  }
  throw new Error("Full navigation did not open");
};
const openAgentSettings = async (section) => {
  await closeNavigationDialog();
  await click("用户菜单");
  const item = await browser.$(
    `//*[@role="menuitem" and normalize-space(.)=${JSON.stringify(section)}]`,
  );
  await wait(() => item.isDisplayed());
  await item.click();
};
const openHistory = async () => {
  if (!(await browser.$(".rss-session-history:not([hidden])").isExisting()))
    await click("历史会话");
  await wait(() =>
    browser.$(".rss-session-history:not([hidden])").isDisplayed(),
  );
};
const assertEmptyHistory = async () => {
  await openHistory();
  await text("还没有已读取的会话");
  assert.equal(
    await browser.$$(".rss-session-history-item").then((items) => items.length),
    0,
    "entering chat does not create an empty session",
  );
  await click("关闭历史会话");
};
const navigate = async (name) => {
  await closeNavigationDialog();
  await expandNavigation();
  await click(name, '//*[@aria-label="主导航"]');
  await browser.execute(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
};
let lastPromptKey;
const completedReply = async (value) => {
  assert.ok(lastPromptKey, "an original submitted prompt is required");
  await wait(() => browser.execute(nativeReplyComplete, lastPromptKey, value));
  await browser.execute(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
};
const prompt = async (value) => {
  const prior = await browser.execute(() =>
    [
      ...document.querySelectorAll(
        '.rss-assistant-timeline article[data-message-kind="user"]',
      ),
    ].map((row) => row.getAttribute("data-message-key")),
  );
  await browser.$(".rss-composer textarea").click();
  await browser.$(".rss-composer textarea").setValue(value);
  assert.equal(
    await browser.execute(
      () =>
        document.activeElement ===
        document.querySelector(".rss-composer textarea"),
    ),
    true,
    "composer must own DOM focus before submission",
  );
  await wait(() =>
    browser.$('.rss-composer button[aria-label="发送消息"]').isEnabled(),
  );
  // Send through the visible product action. Physical Return delivery depends on the
  // interactive desktop's input source/foreground; do not resend an ambiguous command.
  await browser.$('.rss-composer button[aria-label="发送消息"]').click();
  await wait(() =>
    browser.execute(
      (value) =>
        document.querySelector(".rss-composer textarea").value !== value,
      value,
    ),
  );
  lastPromptKey = await wait(() =>
    browser.execute(
      (value, prior) => {
        const row = [
          ...document.querySelectorAll(
            '.rss-assistant-timeline article[data-message-kind="user"]',
          ),
        ].find(
          (row) =>
            !prior.includes(row.getAttribute("data-message-key")) &&
            row.textContent.includes(value),
        );
        return row?.getAttribute("data-message-key");
      },
      value,
      prior,
    ),
  );
};
const selectValue = async (element, label) => {
  // Exercise the controlled Ant Design Select through its visible dropdown.
  await element.click();
  const option = await wait(async () => {
    for (const candidate of await browser.$$(
      ".ant-select-dropdown:not(.ant-select-dropdown-hidden) .ant-select-item-option",
    )) {
      if (
        (await candidate.getText()).trim() === label &&
        (await candidate.isDisplayed())
      )
        return candidate;
    }
  });
  await option.click();
  await wait(
    async () =>
      !(await browser
        .$(".ant-select-dropdown:not(.ant-select-dropdown-hidden)")
        .isDisplayed()),
  );
};
const permission = async (allow) => {
  await wait(async () => {
    assert.equal(
      await visibleText("AI Host 已退出"),
      false,
      "AI Host exited before permission",
    );
    return browser.$('[aria-label="本次工具请求"]').isDisplayed();
  });
  await click(
    allow ? "仅允许本次请求" : "拒绝本次请求",
    '//*[@aria-label="本次工具请求"]',
  );
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
// The live store is exclusive. Observe the actual fixed native launcher, not its locked database.
const readWorkerScopes = async () => {
  if (!scopeRuntime || !receipt?.pid) return [];
  let children = processChildren(receipt.pid);
  const hosts = children.filter(
    (pid) => processIdentity(pid, "comm") === scopeRuntime.node,
  );
  assert.ok(hosts.length <= 1, "one actual owned Host");
  if (!hosts.length) return [];
  children = processChildren(hosts[0]);
  return children
    .filter((pid) => processIdentity(pid, "comm") === scopeRuntime.launcher)
    .map((pid) => {
      assert.equal(Number(processIdentity(pid, "pgid")), pid);
      const launchId = / launch ([a-f0-9-]{36})$/.exec(
        processIdentity(pid, "args"),
      )?.[1];
      assert.ok(launchId, "fixed native launch identity");
      return {
        phase: "observedNative",
        launchId,
        scope: { kind: "processGroup", root: pid },
      };
    });
};
let faultScopeConfirmed = true;
const verifyUnknownFence = async () => {
  const source = `
    import assert from 'node:assert/strict';
    import {setTimeout as delay} from 'node:timers/promises';
    import {randomUUID} from 'node:crypto';
    import {pathToFileURL} from 'node:url';
    import {join} from 'node:path';
    const input=JSON.parse(process.argv[1]);
    const {openSqliteStore}=await import(input.modules.store);
    const {createHost}=await import(input.modules.host);
    const {scopeAbsentWithin,processWorkerAdapter}=await import(input.modules.process);
    const {fixtureSession,acceptance}=await import(pathToFileURL(join(input.root,'packages/ai-contract/dist/testing/index.js')));
    const {fixturePersistence}=await import(pathToFileURL(join(input.root,'tests/ai-host/harness.mjs')));
    const budget=()=>({timeoutMs:5000,signal:new AbortController().signal});
    const {processField:field}=await import(pathToFileURL(join(input.root,'scripts/desktop-dev-process.mjs')));
    const unwrap=(result)=>{assert.equal(result.ok,true,JSON.stringify(result));return result.value};
    // Existing launch-fence fault seam, isolated from the UI's credential namespace.
    const {scope,anchor}=input.actor;
    const child=anchor.pid,birth=anchor.start;
    let store,host;
    assert.equal(field(child,'lstart'),birth);
    assert.equal(Number(field(child,'pgid')),scope.root);
    try {
      store=unwrap(await openSqliteStore({path:input.path,mode:'create'}));
      const session=fixtureSession();unwrap(await store.create(session));unwrap(await store.accept(acceptance(session)));
      const launchId=randomUUID();
      unwrap(await store.reserveLaunch({runtimeDigest:input.runtime.manifestDigest,namespace:session.namespace,
        launchId,artifact:pathToFileURL(join(input.root,'tests/ai-host/provider.mjs')).href,phase:'reserved'}));
      unwrap(await store.registerLaunch(session.namespace,launchId,scope));
      process.kill(scope.root,'SIGKILL');
      const deadline=Date.now()+2000;
      while(field(scope.root,'comm')){assert.ok(Date.now()<deadline);await delay(10);}
      assert.equal(await scopeAbsentWithin(input.runtime,scope,budget()),false);process.kill(child,0);
      await store.close(budget());store=unwrap(await openSqliteStore({path:input.path,mode:'open'}));
      let resolves=0;
      host=unwrap(await createHost({credentialPersistence:fixturePersistence(store),workers:processWorkerAdapter(input.runtime),
        delivery:null,store,launchFences:store,resolve:async()=>{resolves++;throw Error('unknown scope must block restore')}}));
      const retained=unwrap(await store.launches()).find(row=>row.launchId===launchId);
      assert.ok(retained);assert.deepEqual(retained.scope,scope);assert.equal(resolves,0);
      assert.equal(unwrap(await store.snapshotPage(session.namespace,{limit:256})).session.status,'recovery_required');
      process.kill(child,0);await host.close(budget());host=null;process.kill(child,0);
      assert.equal(field(child,'lstart'),birth);assert.equal(Number(field(child,'pgid')),scope.root);
      process.kill(-scope.root,'SIGKILL');
      assert.equal(await scopeAbsentWithin(input.runtime,scope,budget()),true);
      console.log(JSON.stringify({actor:'existing launch-fence fault seam',retained,resolveCalls:resolves,
        descendantObserved:true,originalParentExited:true,scopeAbsentAfterOwnedCleanup:true}));
    } finally {
      try {await host?.close(budget());}
      finally {await store?.close(budget());}
      // The external creator owns actor cleanup even when resource closure throws.
    }
  `;
  let entry;
  return runOwnedScopeProbe(
    scopeRuntime.node,
    source,
    {
      root,
      path: join(directory, "scope-fault.sqlite"),
      actorRecoveryPath: join(directory, "scope-fault-actor.json"),
      runtime: scopeRuntime,
      modules: productModules,
    },
    async (proof, executorPid) => {
      assert.equal(proof.kind, "ownedScope");
      assert.equal(proof.scope.kind, "processGroup");
      assert.ok(Number.isSafeInteger(proof.scope.root) && proof.scope.root > 1);
      assert.ok(Number.isSafeInteger(proof.anchor.pid) && proof.anchor.pid > 1);
      entry = {
        scope: proof.scope,
        anchor: proof.anchor,
        launchId: "fault-" + nonce,
      };
      ownedWorkerScopes.set(entry.scope.root, entry);
      assert.equal(
        processIdentity(proof.scope.root, "comm"),
        scopeRuntime.node,
      );
      assert.equal(
        Number(processIdentity(proof.scope.root, "ppid")),
        executorPid,
      );
      assert.equal(
        processIdentity(proof.anchor.pid, "lstart"),
        proof.anchor.start,
      );
      assert.equal(
        Number(processIdentity(proof.anchor.pid, "pgid")),
        proof.scope.root,
      );
    },
    async () => {
      if (!entry) return;
      const absent = () =>
        scopeAbsentWithin(scopeRuntime, entry.scope, {
          timeoutMs: 1000,
          signal: new AbortController().signal,
        });
      if (!(await absent())) terminateObservedScope(entry);
      await wait(absent);
      faultScopeConfirmed = true;
    },
    faultAbort.signal,
  );
};

const originalRequest = (row) =>
  row.action.kind === "policy"
    ? row.action.choice.request
    : row.action.offer.request;
const originalRequests = (reply) =>
  new Set(
    reply.items
      .filter((item) => item.reference.kind === "request")
      .map((item) => item.reference.request),
  );
const taskButton = (reference) => {
  const key = JSON.stringify(
    reference.kind === "request"
      ? { kind: reference.kind, request: reference.request }
      : reference.kind === "sampling"
        ? {
            kind: reference.kind,
            query: {
              id: reference.query.id,
              revision: reference.query.revision,
            },
          }
        : { kind: reference.kind, id: reference.id },
  );
  return browser.$(
    `.rss-task-table button[data-task-reference=${JSON.stringify(key)}]`,
  );
};
const openNativeTask = async (detail) => {
  await navigate("任务中心");
  await selectValue(await field("任务来源"), "全部来源");
  let selectedView,
    readRetried = false;
  await wait(async () => {
    if (await taskButton(detail.reference).isDisplayed()) return true;
    const alert = await browser.$('.rss-tasks-page [role="alert"]');
    const refresh = await browser.$(
      '//button[normalize-space(.)="刷新原记录"]',
    );
    if (
      selectedView &&
      (await alert.isDisplayed()) &&
      (await refresh.isEnabled())
    ) {
      if (readRetried)
        throw Error(`original task read failed: ${await alert.getText()}`);
      readRetried = true;
      await refresh.click(); // Read the retained task, never resubmit it.
    }
    const current =
      detail.reference.kind === "activity"
        ? detail
        : (await serviceCall("query")).items.find((item) =>
            isDeepStrictEqual(item.reference, detail.reference),
          );
    assert.ok(current, "the authenticated owner must retain the original task");
    const view =
      current.reference.kind === "activity"
        ? "管理活动"
        : current.active
          ? "正在进行"
          : "历史记录";
    if (view !== selectedView) {
      await click(view, '//*[contains(@class,"rss-secondary-nav")]');
      selectedView = view;
    }
    return false;
  });
  await taskButton(detail.reference).click();
  await wait(() => browser.$(".rss-task-detail").isDisplayed());
  assert.ok(
    (await browser.$(".rss-task-detail").getText()).includes(
      detail.reference.request ??
        detail.reference.id ??
        detail.reference.query.id,
    ),
  );
};
const openPageAssistant = async () => {
  await click("页面助手");
  await wait(() =>
    browser.$(".rss-page-assistant .rss-composer textarea").isDisplayed(),
  );
};
const taskExecutions = (page) =>
  page.items
    .filter((item) => item.facts.kind === "execution")
    .map((item) => item.facts.value);
const taskPreparations = (page) =>
  page.items
    .filter((item) => item.facts.kind === "preparation")
    .map((item) => item.facts.value);
const task = async () => {
  const reply = await serviceCall("query");
  const request = fixture.facts.request;
  const existing = taskExecutions(reply).find(
    (row) => row.status.operationRequestId === request,
  );
  if (existing) return { plan: existing.action, snapshot: existing.status };
  const pending = taskPreparations(reply).find(
    (row) => originalRequest(row) === request,
  );
  return (
    pending && {
      pending,
      snapshot: { attempts: 0, cancelRequested: pending.state === "cancelled" },
    }
  );
};
try {
  result.acceptanceEnvironment = {
    platform: process.platform,
    architecture: process.arch,
    ...toolchainEvidence(root),
  };
  if (!candidatePath) result.developmentSource = sourceEvidence(root);
  assert.equal(process.platform, "darwin");
  assert.equal(process.arch, "arm64");
  if (!visualOnly && !controlledService)
    throw new Error(
      "real execution acceptance requires explicit --controlled-service; no service is installed by ordinary UI development",
    );
  keychainState = visualOnly ? undefined : systemKeychains();
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
  assert.equal(
    result.acceptanceEnvironment.node,
    `v${packageManifest.engines.node}`,
  );
  assert.equal(
    result.acceptanceEnvironment.pnpm,
    packageManifest.packageManager.split("@")[1],
  );
  // Temporary assertion owned by this run; never changes lock settings or unlocks a desktop.
  awake = spawn("/usr/bin/caffeinate", ["-diu", "-w", String(process.pid)], {
    stdio: "ignore",
  });
  awake.on("error", (error) => {
    spawnError = error;
  });
  fixture = visualOnly ? undefined : await startModelFixture();
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
    result.mode = "pnpm-dev-main-visual";
    result.provider = "FakeHost renderer fixture";
    result.executor =
      "fixture-none; isolated interaction state and Rust-generated projections";
    result.productionExecution =
      "absent in fixture assembly; real service evidence is separate";
  }
  const baseConfig = JSON.parse(
    readFileSync(join(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
  );
  const nativeConfig = {
    build: {
      devUrl: `http://127.0.0.1:${webPort}`,
      beforeDevCommand: `pnpm dev:web --port ${webPort}`,
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
  if (visualOnly) {
    mark("launch root pnpm dev");
    child = spawn(
      "pnpm",
      [
        "dev",
        ...(visualOnly ? ["--fixture"] : []),
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
  } else {
    const candidate = candidatePath
      ? verifyCandidate(candidatePath)
      : undefined;
    if (candidate)
      assert.ok(
        candidate.binaries.desktop && candidate.runtime,
        "desktop and runtime candidate required",
      );
    assert.ok(
      !security || candidate?.binaries.securityProbe,
      "security requires a frozen native probe",
    );
    result.candidate = candidate;
    mark(
      candidate
        ? "verify prebuilt frozen candidate"
        : "prepare fixed native production candidate",
    );
    for (const [command, args] of candidate
      ? []
      : [
          ["pnpm", ["build:ai-access"]],
          ["cargo", ["build", "--locked", "--release", "-p", "rss-mdm-cli"]],
          ["pnpm", ["--filter", "@rss-mdm-agent/desktop", "build"]],
          [
            process.execPath,
            [
              join(root, "scripts/desktop-dev-runtime.mjs"),
              "--prepare",
              root,
              join(root, ".local-ci-runs/ai-host-dev-runtime"),
              "build",
            ],
          ],
          ["pnpm", ["desktop:build", "--native-acceptance"]],
          [
            "cargo",
            [
              "build",
              "--locked",
              "--release",
              "-p",
              "agent-service",
              "--example",
              "controlled-backend",
              "--bin",
              "rss-execution-service",
            ],
          ],
        ])
      assert.equal(
        await runPreparation(command, args, root, env),
        0,
        "native candidate preparation failed",
      );
    productModules = nativeProductModules(root, candidate?.runtime.path);
    ({ scopeAbsentWithin } = await import(productModules.process));
    mark(
      "explicit controlled installation; native administrator authorization",
    );
    const serviceEvidence = join(
      reports,
      `native-service-${nonce.slice(0, 8)}`,
    );
    result.serviceEvidence = serviceEvidence;
    serviceWorker = spawn(
      "/usr/bin/python3",
      [
        join(root, "scripts/service/verify-execution-macos.py"),
        "--binary",
        candidate?.binaries.service.path ??
          join(cargoTargetDir(root), "release/rss-execution-service"),
        "--cli",
        candidate?.binaries.cli.path ??
          join(cargoTargetDir(root), "release/rssmdm"),
        "--backend",
        candidate?.binaries.backend.path ??
          join(cargoTargetDir(root), "release/examples/controlled-backend"),
        "--desktop",
        candidate?.binaries.desktop.path ??
          join(cargoTargetDir(root), "debug/rss-mdm-desktop"),
        "--output",
        serviceEvidence,
        ...(candidatePath ? ["--candidate", candidatePath] : []),
        ...(security ? ["--security"] : []),
        ...(authorizationPasswordFile
          ? ["--authorization-password-file", authorizationPasswordFile]
          : []),
      ],
      { cwd: root, detached: true, stdio: ["pipe", "pipe", "pipe"] },
    );
    serviceWorker.on("error", serviceFailed);
    serviceWorker.stdin.on("error", serviceFailed);
    serviceWorker.on("exit", (code) => {
      if (!serviceFinishing || code !== 0)
        serviceFailed(new Error("controlled service owner exited"));
    });
    serviceWorker.stdout.setEncoding("utf8");
    serviceWorker.stdout.on("data", (chunk) => {
      serviceOutput += chunk;
      for (let newline; (newline = serviceOutput.indexOf("\n")) >= 0; ) {
        const line = serviceOutput.slice(0, newline);
        serviceOutput = serviceOutput.slice(newline + 1);
        try {
          const value = JSON.parse(line);
          if (value.kind === "desktopService") serviceReady = value;
          else if (value.kind === "serviceFailure")
            serviceFailed(new Error(value.error));
          else {
            servicePending.get(value.id)?.resolve(value.value);
            servicePending.delete(value.id);
          }
        } catch {
          logs += line + "\n";
        }
      }
    });
    serviceWorker.stderr.on("data", (chunk) => {
      logs += chunk;
      process.stderr.write(chunk);
    });
    await wait(() => {
      assert.equal(serviceWorker.exitCode, null, logs.slice(-3000));
      return serviceReady;
    }, 180000);
    result.serviceArtifact = serviceReady.service;
    const probe = JSON.parse(
      execFileSync(serviceReady.desktop, ["--service-probe"], {
        encoding: "utf8",
      }),
    );
    assert.equal(probe.phase, "connected");
    assert.equal(probe.status.readiness.phase, "ready");
    assert.equal((await serviceCall("status")).startRequests, 0);
    env.RSS_AI_HOST_RUNTIME =
      candidate?.runtime.path ??
      join(root, ".local-ci-runs/ai-host-dev-runtime");
    result.security = await serviceCall("security");
    result.executor =
      "authenticated unified service; independent effects checked later in this run";
    result.provider = "codex-0.155.0; synthetic loopback model fixture";
    result.keychain = "isolated noninteractive macOS file keychain";
    mark("launch protected fixed production main as the actual login user");
    child = spawn(serviceReady.desktop, ["--test-data-dir", directory], {
      cwd: root,
      env,
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    result.mode = "protected-fixed-main";
    result.protectedDesktop = serviceReady.desktop;
  }
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
        process.stderr.write(
          line
            .replaceAll(nonce, "[redacted-capability]")
            .replaceAll(fixture?.secret ?? "<none>", "[redacted]") + "\n",
        );
        if (line.startsWith("RSS_NATIVE_E2E "))
          receipt = JSON.parse(line.slice(15));
        if (line.startsWith("RSS_AI_HOST_STATUS "))
          hostStatus = JSON.parse(line.slice(19));
      }
    });
  }
  await wait(() => {
    assert.equal(child.exitCode, null, logs.slice(-3000));
    assert.equal(child.signalCode, null, logs.slice(-3000));
    return receipt && (visualOnly || hostStatus);
  }, 600000);
  assert.equal(receipt.nonceSha256, sha256(nonce));
  assert.equal(receipt.port, port);
  assert.equal(receipt.dataRootSha256, sha256(directory));
  if (!visualOnly) {
    assert.equal(hostStatus.phase, "ready");
    assert.equal(hostStatus.source, "development_override");
    assert.ok(
      ["native-e2e.keychain", "native-e2e.keychain-db"].some((name) =>
        existsSync(join(directory, name)),
      ),
      "native acceptance must own an isolated OS keychain",
    );
  } else {
    assert.equal(existsSync(join(directory, "execution.sqlite")), false);
    result.hostPreparation = "not-required";
    result.keychain = "not-accessed";
    result.productionIPC = "absent";
  }
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
  const artifact =
    env.RSS_AI_HOST_RUNTIME ?? join(root, ".local-ci-runs/ai-host-dev-runtime");
  let manifest;
  if (!visualOnly) {
    manifest = verifyNativeRuntime(root, artifact, candidatePath);
    result.runtimeManifestSha256 = sha256(
      readFileSync(join(artifact, "manifest.json")),
    );
    scopeRuntime = {
      launcher: join(artifact, "bin/rss-ai-worker-launcher"),
      node: join(artifact, "bin/node"),
      manifestDigest: sha256(
        readFileSync(join(artifact, "worker-manifest.json")),
      ),
      runtimeTreeSha256: manifest.runtimeTreeSha256,
    };
  }
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
    "driver main must descend from this run owner",
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
  const initialHostPid = visualOnly ? undefined : hostPid();
  if (!visualOnly) assert.ok(initialHostPid);
  result.owner = {
    mainPid: receipt.pid,
    hostPid: initialHostPid,
    mainBinarySha256: sha256(readFileSync(processField(receipt.pid, "comm"))),
    hostGeneration: hostStatus?.generation,
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
    result.mode = "pnpm-dev-main-react-visual";
    result.modelFixture = false;
    result.executor =
      "not exercised; React foundation and state with fixture Host";
    mark("React foundation in the product WKWebView");
    await text("组件基础验收");
    await click("查询原请求");
    await text("查询次数：1");
    await click("更多操作");
    const detailsItem = await browser.$('[role="menuitem"]');
    await wait(() => detailsItem.isDisplayed());
    await detailsItem.click();
    await wait(() => browser.$('[role="dialog"]').isDisplayed());
    await browser.$('[aria-label="详情字段"]').setValue("原生草稿");
    key(53);
    await wait(async () => !(await browser.$('[role="dialog"]').isDisplayed()));
    await wait(() =>
      browser.execute(() =>
        document.activeElement?.textContent?.includes("更多操作"),
      ),
    );
    assert.equal(
      await browser.execute(() =>
        document.activeElement?.textContent?.includes("更多操作"),
      ),
      true,
    );
    const message = await browser.$('[aria-label="消息"]');
    await message.setValue("原生消息");
    await click("发送消息");
    await text("原生消息");
    assert.equal(
      await browser.execute(
        () =>
          document.querySelectorAll(
            ".rss-markdown img,.rss-markdown script,.rss-markdown a",
          ).length,
      ),
      0,
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-foundations.png"),
    );
    await browser.execute(() => {
      location.search = "?state";
    });
    await text("业务状态验收");
    await wait(
      async () =>
        (await browser.$('[data-testid="state-connection"]').getText()) ===
        "connected",
    );
    await browser.$('[aria-label="状态层草稿"]').setValue("原生状态草稿");
    await click("选择软件更新");
    for (const expected of ["1:rail", "2:full", "3:rail", "0:hidden"]) {
      await click("切换布局");
      await wait(
        async () =>
          (await browser.$('[data-testid="state-layout"]').getText()) ===
          expected,
      );
    }
    await click("隐藏或恢复侧栏");
    await wait(
      async () =>
        (await browser.$('[data-testid="state-layout"]').getText()) ===
        "3:rail",
    );
    await click("切换布局");
    await wait(
      async () =>
        (await browser.$('[data-testid="state-layout"]').getText()) ===
        "0:hidden",
    );
    await click("读取软件");
    await wait(
      async () =>
        (await browser.$('[data-testid="state-inventory"]').getText()) === "4",
    );
    await click("回到助手");
    assert.equal(
      await browser.$('[aria-label="状态层草稿"]').getValue(),
      "原生状态草稿",
    );
    assert.ok(
      Number(await browser.$('[data-testid="state-sessions"]').getText()) > 0,
    );
    await browser.saveScreenshot(join(reports, "desktop-native-state.png"));
    result.checks.push(
      "React state owner and real client/test Host in WKWebView",
      "directed layout cycle retains session draft and async inventory",
      "React foundation in macOS WKWebView",
      "drawer Escape and focus restoration",
      "controlled message input",
      "inert Markdown",
    );
    mark("v4 shell and home in the product WKWebView");
    await browser.execute(() => {
      location.search = "?desktop";
    });
    await wait(() => browser.$('[aria-label="查看 Workspace"]').isDisplayed());
    mark("v4 shell directed cycle");
    const layout = () =>
      browser.execute(() =>
        document
          .querySelector(".rss-desktop-shell")
          ?.getAttribute("data-layout-step"),
      );
    await wait(async () => (await layout()) === "0");
    const origin = await browser.execute(() => {
      const bounds = document
        .querySelector('[aria-label="切换侧栏布局"]')
        .getBoundingClientRect();
      return { x: bounds.x, y: bounds.y };
    });
    for (const step of [1, 2, 3, 0]) {
      await click("切换侧栏布局");
      await wait(async () => (await layout()) === String(step));
      const position = await browser.execute(() => {
        const bounds = document
          .querySelector('[aria-label="切换侧栏布局"]')
          .getBoundingClientRect();
        return { x: bounds.x, y: bounds.y };
      });
      assert.deepEqual(position, origin);
    }
    await browser
      .$('[aria-label="主页助手草稿"]')
      .setValue("WKWebView 原生草稿");
    mark("native Command-K and Enter global search");
    key(40, false, true);
    await wait(() =>
      browser
        .$('[role="combobox"][aria-label="搜索应用、工具和设置"]')
        .isDisplayed(),
    );
    await browser
      .$('[role="combobox"][aria-label="搜索应用、工具和设置"]')
      .setValue("Workspace");
    key(36);
    await wait(async () => !(await browser.$('[role="dialog"]').isDisplayed()));
    await click("主页", "//div[contains(@class,'rss-hidden-primary')]");
    assert.equal(
      await browser.$('[aria-label="主页助手草稿"]').getValue(),
      "WKWebView 原生草稿",
    );
    mark("native page-assistant Escape and focus");
    await click("页面助手");
    await wait(() => browser.$('[aria-label="关闭页面助手"]').isDisplayed());
    key(53);
    await wait(() =>
      browser.execute(
        () => document.activeElement?.getAttribute("aria-label") === "页面助手",
      ),
    );
    await browser.execute(() => {
      document.querySelector(".fixture-controls").style.display = "none";
    });
    await browser.saveScreenshot(
      join(reports, "desktop-native-shell-home.png"),
    );
    result.checks.push(
      "v4 shell/home uses the product macOS WKWebView",
      "fixed RSS toggle coordinates through 0-1-2-3-0",
      "native Command-K/Enter search and Escape page-assistant focus",
      "native shell navigation retains original assistant draft",
    );
    mark("v4 business pages in the product WKWebView, explicit sample ports");
    for (const [name, ready, file] of [
      ["软件中心", '.rss-store-grid [aria-label="查看 工作空间"]', "software"],
      ["系统设置", ".rss-system-banner", "system"],
      ["工具中心", ".rss-tool-grid", "tools"],
    ]) {
      await click(name, "//div[contains(@class,'rss-hidden-primary')]");
      if (name === "软件中心")
        await click("发现", "//div[contains(@class,'rss-hidden-secondary')]");
      await wait(() => browser.$(ready).isDisplayed());
      await browser.execute(async () => {
        await document.fonts.ready;
        await new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        );
      });
      await browser.saveScreenshot(
        join(reports, `desktop-native-pages-${file}.png`),
      );
    }
    mark("native staged network inspection and Escape");
    await click("开始检查");
    await wait(() => browser.$('[aria-label="网络检测主机"]').isDisplayed());
    assert.equal(
      await browser.execute(() =>
        window.rssPageFixture.events.some(
          (event) => event.operation === "network_run",
        ),
      ),
      false,
    );
    key(53);
    await wait(
      async () =>
        !(await browser.$('[aria-label="网络检测主机"]').isDisplayed()),
    );
    mark("v4 task center and original cancellation in WKWebView");
    const previousDocument = await browser.execute(
      () => performance.timeOrigin,
    );
    await browser.execute(() => {
      location.search = "?desktop&scene=tasks";
    });
    await wait(() =>
      browser.execute(
        (before) =>
          performance.timeOrigin !== before &&
          !!window.rssPageFixture &&
          !!document.querySelector(".fixture-controls"),
        previousDocument,
      ),
    );
    await wait(() => browser.$('[aria-label="切换侧栏布局"]').isDisplayed());
    await browser.execute(() => {
      document.querySelector(".fixture-controls").style.display = "none";
    });
    await click("任务中心", "//div[contains(@class,'rss-hidden-primary')]");
    await wait(() =>
      browser.$(".rss-task-table button[data-task-reference]").isDisplayed(),
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-tasks-active.png"),
    );
    await browser.$(".rss-task-table button[data-task-reference]").click();
    await wait(() => browser.$(".rss-task-detail .ant-timeline").isDisplayed());
    await click("请求取消");
    await wait(() => browser.$(".ant-modal .ant-btn-primary").isDisplayed());
    await browser.$(".ant-modal .ant-btn-primary").click();
    await wait(() =>
      browser.execute(
        () =>
          window.rssDesktop.state.workspace.tasks.state.selected?.facts.value
            .status.cancelRequested === true,
      ),
    );
    assert.equal(
      await browser.execute(
        () =>
          window.rssDesktop.state.workspace.tasks.state.selected.facts.value
            .status.phase,
      ),
      "running",
    );
    assert.equal(
      await browser.execute(
        () =>
          window.rssPageFixture.events.filter(
            (event) => event.operation === "task_cancel",
          ).length,
      ),
      1,
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-task-cancel-requested.png"),
    );
    key(53);
    await wait(
      async () => !(await browser.$(".rss-task-detail").isDisplayed()),
    );
    result.checks.push(
      "v4 task list, real owner timeline and original control in WKWebView; acknowledged cancellation remains running until separate termination/effect evidence",
    );
    mark(
      "React assistant history, official input and trusted one-action card in WKWebView",
    );
    await click("AI 助手", "//div[contains(@class,'rss-hidden-primary')]");
    await click("新会话", "//div[contains(@class,'rss-hidden-secondary')]");
    await browser
      .$('.rss-assistant-input textarea[aria-label="消息"]')
      .setValue("WKWebView 助手原生输入验收");
    key(36);
    await wait(() =>
      browser.execute(
        () =>
          !!window.rssDesktop.state.workspace.assistant.state.selected &&
          window.rssDesktop.state.workspace.assistant.busy(),
      ),
    );
    const assistantSession = await browser.execute(
      () => window.rssDesktop.state.workspace.assistant.state.selected,
    );
    const driveAssistant = async (input) => {
      const value = await browser.execute(
        async (input) => {
          const response = await fetch("/__fixture/assistant", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(input),
          });
          return { ok: response.ok, body: await response.text() };
        },
        { sessionId: assistantSession, ...input },
      );
      assert.equal(
        value.ok,
        true,
        `assistant scripted Host stimulus ${input.action}`,
      );
      return JSON.parse(value.body);
    };
    await driveAssistant({ action: "surface" });
    await wait(() =>
      browser.$('.rss-ai-surface input[aria-label="Answer"]').isDisplayed(),
    );
    await browser
      .$('.rss-ai-surface input[aria-label="Answer"]')
      .setValue("WKWebView 官方输入绑定");
    await browser
      .$('.rss-assistant-input textarea[aria-label="消息"]')
      .setValue("本会话未提交草稿");
    await click("历史会话", "//div[contains(@class,'rss-hidden-secondary')]");
    await wait(() =>
      browser.$(".rss-session-history:not([hidden])").isDisplayed(),
    );
    await browser.$('.rss-session-history-item[aria-current="page"]').click();
    assert.equal(
      await browser.$(".rss-session-history:not([hidden])").isDisplayed(),
      true,
    );
    assert.equal(
      await browser
        .$('.rss-assistant-input textarea[aria-label="消息"]')
        .getValue(),
      "本会话未提交草稿",
    );
    await click("关闭历史会话");
    await click("主页", "//div[contains(@class,'rss-hidden-primary')]");
    await click("AI 助手", "//div[contains(@class,'rss-hidden-primary')]");
    assert.equal(
      await browser.$('.rss-ai-surface input[aria-label="Answer"]').getValue(),
      "WKWebView 官方输入绑定",
    );
    await click("Send answer");
    await wait(() =>
      browser.execute(() =>
        document
          .querySelector(".rss-ai-surface")
          ?.textContent.includes("此问题已经回答。"),
      ),
    );
    await driveAssistant({
      action: "advance",
      events: [{ type: "terminal", outcome: "completed" }],
    });
    await wait(() =>
      browser.execute(
        () => !window.rssDesktop.state.workspace.assistant.busy(),
      ),
    );
    await browser
      .$('.rss-assistant-input textarea[aria-label="消息"]')
      .setValue("WKWebView 准备本次修复");
    key(36);
    await wait(() =>
      browser.execute(() => window.rssDesktop.state.workspace.assistant.busy()),
    );
    const operationId = await browser.execute((sessionId) => {
      const id = crypto.randomUUID();
      window.rssPageFixture.stageAssistantOperation(sessionId, id);
      return id;
    }, assistantSession);
    await driveAssistant({
      action: "delivery",
      operationId,
      proposal: { name: "execution_operation_prepare", arguments: {} },
    });
    await wait(() =>
      browser
        .$('.rss-operation-card input[aria-label="修复名称"]')
        .isDisplayed(),
    );
    await browser
      .$('.rss-operation-card input[aria-label="修复名称"]')
      .setValue("原生确认的最终参数");
    await browser.saveScreenshot(
      join(reports, "desktop-native-assistant-operation.png"),
    );
    await click("确认本次具体动作");
    await wait(() =>
      browser.execute(
        () =>
          window.rssPageFixture.events.filter(
            (e) => e.operation === "ai_operation_submit",
          ).length === 1,
      ),
    );
    assert.equal(
      await browser.execute(
        (id) =>
          window.rssPageFixture.records.get(id).action.choice.parameters.name,
        operationId,
      ),
      "原生确认的最终参数",
    );
    assert.equal(
      await browser.execute(
        (id) => window.rssPageFixture.records.get(id).state,
        operationId,
      ),
      "submitting",
    );
    assert.equal(await browser.$('[role="dialog"]').isExisting(), false);
    await driveAssistant({
      action: "advance",
      events: [{ type: "terminal", outcome: "completed" }],
    });
    await browser.saveScreenshot(
      join(reports, "desktop-native-assistant-submitted.png"),
    );
    result.checks.push(
      "production assistant in macOS WKWebView: native keyboard input, fixed secondary history keeps per-session draft, official A2UI binding/answer, original trusted Policy draft edited and submitted once; scripted Host/sample record, no real model or OS execution effect claimed",
    );
    mark("native redacted diagnostic snapshot");
    await click("用户菜单");
    const diagnosticMenu =
      '//*[@role="menuitem" and normalize-space(.)="运行与诊断"]';
    await wait(() => browser.$(diagnosticMenu).isDisplayed());
    await browser.$(diagnosticMenu).click();
    await click("预览脱敏诊断");
    await wait(() => browser.$(".rss-diagnostic-preview").isDisplayed());
    await browser.saveScreenshot(
      join(reports, "desktop-native-pages-diagnostic.png"),
    );
    key(53);
    await wait(() =>
      browser.execute(
        () =>
          window.rssDesktop.state.workspace.diagnostics.state.preview == null,
      ),
    );
    result.checks.push(
      "v4 software covers, system overview and utility cards in WKWebView",
      "native Escape closes staged network check without running it",
      "native diagnostic preview uses and discards the original redacted sample snapshot; no OS export effect claimed",
    );
    if (!baseline) {
      result.visual = [];
      await navigate("AI 助手");
      await click("新会话");
      await browser.execute(() => {
        window.nativeVisualInput = document.querySelector(
          ".rss-assistant-input textarea",
        );
      });
      await browser
        .$(".rss-assistant-input textarea")
        .setValue("原生尺寸与主题草稿");
      const originalDark =
        script(
          'tell application "System Events" to tell appearance preferences to get dark mode',
        ) === "true";
      try {
        for (const dark of [false, true]) {
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
          for (const [width, height] of [
            [1100, 760],
            [1440, 960],
            [480, 400],
          ]) {
            await browser.setWindowSize(width, height);
            const metrics = await browser.execute(() => {
              const input = document.querySelector(
                  ".rss-assistant-input textarea",
                ),
                rect = input.getBoundingClientRect();
              const visibleComposers = [
                ...document.querySelectorAll(".rss-composer textarea"),
              ].filter((node) => node.getClientRects().length > 0);
              return {
                width: innerWidth,
                height: innerHeight,
                dpr: devicePixelRatio,
                theme: matchMedia("(prefers-color-scheme: dark)").matches
                  ? "dark"
                  : "light",
                inputVisible:
                  rect.top >= 0 &&
                  rect.bottom <= innerHeight &&
                  rect.left >= 0 &&
                  rect.right <= innerWidth,
                overflow: document.documentElement.scrollWidth > innerWidth,
                composerCount: visibleComposers.length,
                sameInput: input === window.nativeVisualInput,
                background: getComputedStyle(
                  document.querySelector(".rss-composer"),
                ).backgroundColor,
              };
            });
            assert.equal(metrics.theme, dark ? "dark" : "light");
            assert.equal(metrics.inputVisible, true);
            assert.equal(metrics.overflow, false);
            assert.equal(metrics.composerCount, 1);
            assert.equal(metrics.sameInput, true);
            assert.equal(
              await browser.$(".rss-assistant-input textarea").getValue(),
              "原生尺寸与主题草稿",
            );
            assert.ok(
              !["transparent", "rgba(0, 0, 0, 0)"].includes(metrics.background),
            );
            result.visual.push(metrics);
            await browser.saveScreenshot(
              join(
                reports,
                `desktop-native-assistant-${metrics.theme}-${width}.png`,
              ),
            );
          }
        }
      } finally {
        script(
          `tell application "System Events" to tell appearance preferences to set dark mode to ${originalDark}`,
        );
        await wait(() =>
          browser.execute(
            (value) =>
              matchMedia("(prefers-color-scheme: dark)").matches === value,
            originalDark,
          ),
        );
        result.systemThemeRestored = true;
      }
      // A physical Shift-Return adds a newline without creating a session.
      await browser.setWindowSize(1100, 760);
      await browser.$(".rss-assistant-input textarea").click();
      const sessionsBefore = await browser.execute(
        () => window.rssDesktop.state.workspace.assistant.sessions().length,
      );
      key(36, true);
      assert.ok(
        (await browser.$(".rss-assistant-input textarea").getValue()).includes(
          "\n",
        ),
      );
      assert.equal(
        await browser.execute(
          () => window.rssDesktop.state.workspace.assistant.sessions().length,
        ),
        sessionsBefore,
      );
      // Page and main composers share the conversation draft; Escape restores the trigger.
      await navigate("软件中心");
      await click("已安装");
      await openPageAssistant();
      await browser
        .$(".rss-page-assistant textarea")
        .setValue("页面助手独立草稿");
      key(48);
      await wait(() =>
        browser.execute(
          () =>
            !!document.activeElement?.closest(".rss-page-assistant") &&
            document.activeElement.tagName !== "TEXTAREA",
        ),
      );
      key(48, true);
      await wait(() =>
        browser.execute(
          () =>
            document.activeElement ===
            document.querySelector(".rss-page-assistant textarea"),
        ),
      );
      key(53);
      await wait(() =>
        browser.execute(
          () =>
            document.activeElement?.getAttribute("aria-label") === "页面助手",
        ),
      );
      await navigate("AI 助手");
      assert.equal(
        await browser.$(".rss-assistant-input textarea").getValue(),
        "页面助手独立草稿",
      );
      result.checks.push(
        "native-six-layout-theme-cases",
        "native-theme-hot-switch",
        "native-context-tab-escape-focus",
        "native-same-conversation-draft-preservation",
        "native-Shift-Return-without-send",
      );
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
        await click("新会话");
        await browser.$(".rss-composer textarea").setValue("");
        await browser.$(".rss-composer textarea").click();
        const before = await browser.execute(
          async () =>
            window.rssDesktop.state.workspace.assistant.sessions().length,
        );
        await browser.execute(() => {
          window.nativeImeEvents = [];
          const input = document.querySelector(".rss-composer textarea");
          window.nativeImeListener = (event) =>
            window.nativeImeEvents.push(event.type);
          for (const type of ["compositionstart", "compositionend"])
            input.addEventListener(type, window.nativeImeListener);
        });
        try {
          await browser.switchToWindow("main");
          await browser.$(".rss-composer textarea").click();
          native("get frontmost");
          selectInputSource("com.apple.inputmethod.SCIM.ITABC");
          assert.equal(
            inputSources().current,
            "com.apple.inputmethod.SCIM.ITABC",
            "owned application's Chinese input source selected",
          );
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
          const confirmed = await browser
            .$(".rss-composer textarea")
            .getValue();
          assert.ok(
            confirmed.length > 0,
            "native Return commits the composing text",
          );
          assert.equal(
            await browser.execute(
              async () =>
                window.rssDesktop.state.workspace.assistant.sessions().length,
            ),
            before,
            "IME confirmation must not submit",
          );
          // Apple Pinyin Return commits raw spelling; Space selects its Chinese candidate.
          await browser.$(".rss-composer textarea").setValue("");
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
            await browser.$(".rss-composer textarea").getValue(),
            /[^\x00-\x7f]/,
            "Space selects the native Chinese candidate",
          );
          assert.equal(
            await browser.execute(
              async () =>
                window.rssDesktop.state.workspace.assistant.sessions().length,
            ),
            before,
            "Chinese candidate confirmation must not submit",
          );
          key(36);
          await wait(
            async () =>
              (await browser.execute(
                async () =>
                  window.rssDesktop.state.workspace.assistant.sessions().length,
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
          await browser.execute(() => {
            const input = document.querySelector(".rss-composer textarea");
            for (const type of ["compositionstart", "compositionend"])
              input?.removeEventListener(type, window.nativeImeListener);
            delete window.nativeImeListener;
          });
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
        reason: "no native page zoom entry; CSS zoom is not evidence",
      };
      result.displayScaling = {
        status: "observed",
        dpr: [...new Set(result.visual.map((row) => row.dpr))],
        otherScales: "unverified",
      };
      result.appearance = {
        status: "fixture",
        reason:
          "visual assembly uses fixed appearance facts; real AppKit policy is checked only in controlled-service mode",
      };
      await browser.closeWindow();
      result.closeWindowAction =
        "WebDriver -> native Tauri window.close; menu reopen";
      result.nativeTitlebarClose =
        "unverified; AXCloseButton unavailable in the unbundled dev window";
      await wait(async () => (await browser.getWindowHandles()).length === 0);
      menu("显示窗口");
      await wait(async () => (await browser.getWindowHandles()).length === 1);
      await browser.switchToWindow((await browser.getWindowHandles())[0]);
      await browser.execute(() => {
        location.search = "?desktop";
      });
      await wait(() => browser.$('[aria-label="切换侧栏布局"]').isDisplayed());
      await navigate("AI 助手");
      await wait(() =>
        browser.$(".rss-assistant-input textarea").isDisplayed(),
      );
      result.checks.push("native-visual-close-reopen");
    }
    result.baseline = baseline;
    result.windowsAppearance = "unverified; no Windows host";
    assert.deepEqual(
      sourceEvidence(root),
      result.developmentSource,
      "source changed during acceptance",
    );
    result.status = "passed";
  } else {
    mark(
      "current authenticated local workspace and saved unverified connection",
    );
    if (
      await browser
        .$('//button[normalize-space(.)="使用本机工作区"]')
        .isDisplayed()
    )
      await click("使用本机工作区");
    await wait(() => browser.$(".rss-desktop-shell").isDisplayed());
    await openAgentSettings("账号与组织");
    await text("当前使用本机工作区");
    // Test-user switching was retired with the old UI. Use the actual OS owner;
    // cross-user enforcement is exercised by the native service security probe.
    result.callerIsolation = {
      status: "not_verified",
      reason:
        "no synthetic user switch in the production UI; service cross-user security requires --security",
    };
    mark("native task port reads before model setup or any execution");
    await navigate("任务中心");
    await wait(
      async () =>
        (await browser.$(".rss-task-table").isDisplayed()) ||
        (await visibleText("当前视图没有任务")),
    );
    result.checks.push("native-task-read-before-model-or-execution");
    await openAgentSettings("个人 Passkey");
    await text("密码输入与用户验证只在受保护的原生对话框中进行。");
    assert.equal(
      await browser
        .$('.rss-preferences-page input[type="password"]')
        .isExisting(),
      false,
    );
    result.checks.push("personal-vault-native-management-no-renderer-password");
    await openAgentSettings("AI 与连接");
    await text("还没有 AI 连接");
    await click("添加连接");
    await (await field("连接名称")).setValue("Golden Codex");
    await selectValue(await field("AI 配置来源"), "自定义 API");
    await (await field("AI API 地址")).setValue(fixture.url);
    await (await field("AI 模型")).setValue("native-golden-model");
    await selectValue(await field("AI 连接用途"), "受控工具");
    await (await field("AI 新密钥")).setValue(fixture.secret);
    await click("保存连接");
    await text("待验证");
    assert.equal(
      await browser.$('input[aria-label="AI 新密钥"]').isExisting(),
      false,
      "secret form discarded after save",
    );
    assert.equal(fixture.facts.requests, 0, "saving must not contact model");
    result.checks.push(
      "saved-unverified",
      "noninteractive-isolated-keychain",
      "secret-cleared",
    );
    mark("failed probe then explicit retry through the saved configuration");
    await click("配置 Golden Codex");
    await click("验证当前已保存配置");
    await text("验证未通过");
    assert.ok(fixture.facts.rejectedAuthentication > 0);
    fixture.accept();
    if (!(await browser.$('[role="dialog"]').isDisplayed()))
      await click("配置 Golden Codex");
    await click("验证当前已保存配置");
    await text("已验证");
    await navigate("AI 助手");
    assert.equal(
      await browser.$(".rss-assistant-input textarea").getValue(),
      "",
    );
    await assertEmptyHistory();
    result.checks.push("failed-probe-retry", "blank-composer");
    await browser.saveScreenshot(
      join(reports, "desktop-native-start-default.png"),
    );
    mark("v4 narrow navigation overlay and fixed RSS control");
    await browser.setWindowSize(480, 400);
    await expandNavigation();
    assert.equal(
      await browser.execute(() => {
        const sidebar = document
          .querySelector(".rss-sidebar")
          .getBoundingClientRect();
        const toggle = document
          .querySelector('[aria-label="切换侧栏布局"]')
          .getBoundingClientRect();
        return (
          sidebar.left >= 0 &&
          sidebar.right <= innerWidth &&
          toggle.left >= 0 &&
          toggle.right <= innerWidth
        );
      }),
      true,
    );
    await browser.saveScreenshot(
      join(reports, "desktop-native-navigation.png"),
    );
    key(53);
    await wait(
      async () =>
        (await browser
          .$(".rss-desktop-shell")
          .getAttribute("data-layout-step")) === "0",
    );
    result.checks.push("v4-native-narrow-navigation-overlay");
    mark("human action enters the service without a backend confirmation gate");
    await browser.setWindowSize(1100, 760);
    await navigate("软件中心");
    await click("发现");
    assert.equal(originalRequests(await serviceCall("query")).size, 0);
    const published = serviceReady.catalog;
    // The controlled catalog has exactly one visible resource. Its detail must
    // show the service's original policy identity before a concrete submission.
    await wait(() =>
      browser.$(".rss-store-grid .rss-software-card").isDisplayed(),
    );
    await browser
      .$(
        ".rss-store-grid .rss-software-card .rss-software-card-footer button:last-child",
      )
      .click();
    await wait(() => browser.$('[role="dialog"]').isDisplayed());
    await text(published.policyId);
    await click("确认并提交");
    await wait(async () =>
      taskExecutions(await serviceCall("query")).some(
        (row) => row.status.attempts === 1,
      ),
    );
    fixture.facts.request = taskExecutions(await serviceCall("query")).find(
      (row) => row.status.attempts === 1,
    ).action.requestId;
    mark(
      "human task is visible without an AI conversation, with stable return focus",
    );
    const humanTask = (await serviceCall("query")).items.find(
      (item) =>
        item.reference.kind === "request" &&
        item.reference.request === fixture.facts.request,
    );
    assert.equal(humanTask.source, "human");
    await openNativeTask(humanTask);
    assert.ok((await browser.$$(".rss-task-detail [data-node]")).length > 0);
    await click(
      "关闭",
      '//*[contains(@class,"rss-task-detail") or contains(@class,"ant-drawer")]',
    );
    await wait(() =>
      browser.execute(
        (reference) =>
          document.activeElement?.getAttribute("data-task-reference") ===
          JSON.stringify(reference),
        humanTask.reference,
      ),
    );
    await browser.setWindowSize(480, 400);
    assert.equal(
      await browser.execute(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      true,
    );
    await browser.setWindowSize(1100, 760);
    assert.equal(originalRequests(await serviceCall("query")).size, 1);
    result.checks.push(
      "human-task-without-ai",
      "task-list-native-return-focus",
      "task-center-narrow",
    );
    await navigate("AI 助手");
    mark(
      "first send and read the original human task without a generic permission",
    );
    await prompt("GOLDEN_HELLO 查看办公套件");
    await completedReply("完成 HELLO");
    await browser.$('[aria-label="复制代码"]').click();
    await wait(
      async () =>
        (await visibleText("已复制")) || (await visibleText("复制未完成")),
    );
    result.clipboard = (await visibleText("已复制"))
      ? "writeText"
      : "explicit-selection-fallback";
    result.checks.push("native-copy-or-explicit-fallback");

    assert.equal((await task()).snapshot.attempts, 1);
    assert.equal((await task()).plan.initiator.trigger.kind, "human");
    assert.equal(
      await browser.$('[aria-label="本次工具请求"]').isExisting(),
      false,
    );
    await prompt("GOLDEN_CANCEL_REJECT 拒绝取消");
    await permission(false);
    await completedReply("完成 CANCEL_REJECT");
    assert.equal((await task()).snapshot.cancelRequested, false);
    assert.equal(originalRequests(await serviceCall("query")).size, 1);
    mark("read the original executed task through its product card");
    await prompt("GOLDEN_READ 读取状态");
    await completedReply("完成 READ");
    await openNativeTask(
      (await serviceCall("query")).items.find(
        (item) =>
          item.reference.kind === "request" &&
          item.reference.request === fixture.facts.request,
      ),
    );
    assert.equal(originalRequests(await serviceCall("query")).size, 1);
    await navigate("AI 助手");
    await prompt("GOLDEN_CANCEL_ALLOW 允许取消请求");
    await permission(true);
    await completedReply("完成 CANCEL_ALLOW");
    await wait(async () => (await task())?.snapshot.cancelRequested === true);
    await prompt("GOLDEN_READ 读取状态");
    await completedReply("完成 READ");
    assert.equal(
      await browser.$('[aria-label="本次工具请求"]').isExisting(),
      false,
    );
    result.checks.push(
      "first-send-single-session",
      "cancel-allow-reject",
      "reads-without-approval",
      "human-without-backend-confirmation",
      "human-original-journal-detail",
    );
    mark("manual history position survives a new reply");
    await browser
      .$(".rss-assistant-timeline article:first-child")
      .scrollIntoView();
    const scrollTop = await browser.execute(
      () => document.querySelector(".rss-assistant-scroll").scrollTop,
    );
    await prompt("GOLDEN_HELLO 保留阅读位置");
    await completedReply("完成 HELLO");
    assert.ok(
      Math.abs(
        (await browser.execute(
          () => document.querySelector(".rss-assistant-scroll").scrollTop,
        )) - scrollTop,
      ) < 2,
    );
    await wait(() => browser.$(".rss-latest-output").isDisplayed());
    await click("有新内容 · 回到最新");
    result.checks.push("manual-scroll-preserved", "jump-to-latest");
    mark("narrow layout, native focus and scrolling");
    await browser.setWindowSize(480, 400);
    await expandNavigation();
    await browser.saveScreenshot(
      join(reports, "desktop-native-navigation-narrow.png"),
    );
    key(53);
    await wait(
      async () =>
        (await browser
          .$(".rss-desktop-shell")
          .getAttribute("data-layout-step")) === "0",
    );
    await browser.$(".rss-composer textarea").setValue("保留键盘验收草稿");
    await browser.$(".rss-composer textarea").click();
    assert.equal(
      await browser
        .$('.rss-composer button[aria-label="发送消息"]')
        .isEnabled(),
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
          document.querySelector('.rss-composer button[aria-label="发送消息"]'),
      ),
    );
    key(48, true);
    await wait(() =>
      browser.execute(
        () =>
          document.activeElement ===
          document.querySelector(".rss-composer textarea"),
      ),
    );
    await browser.$(".rss-composer textarea").setValue("");
    assert.equal(
      await browser.execute(
        () =>
          [...document.querySelectorAll('[role="log"]')].filter(
            (node) => node.getClientRects().length > 0,
          ).length,
      ),
      1,
    );
    assert.equal(
      await browser.execute(() => {
        const r = document
          .querySelector(".rss-composer textarea")
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
    await openHistory();
    await wait(() =>
      browser.$(".rss-session-history:not([hidden])").isDisplayed(),
    );
    result.checks.push(
      "native-tab-shift-tab-escape",
      "focus-restoration",
      "narrow-composer",
      "single-live-log",
    );
    assert.equal(
      await browser.execute(
        () => document.querySelectorAll(".rss-session-history-item").length,
      ),
      1,
      "all first-chat operations share one session",
    );
    mark("page context in the same native conversation at three sizes");
    result.completion = await serviceCall("completion", {
      request: fixture.facts.request,
    });
    // Inspect the same published resource through a distinct AI-origin request.
    // An uncertain original execution remains intact; do not clear or resubmit it.
    result.catalog = serviceReady.catalog;
    fixture.facts.catalog = result.catalog;
    assert.equal((await task()).snapshot.attempts, 1);
    mark(
      "a distinct software request requires human confirmation before any Start",
    );
    await prompt("GOLDEN_REVIEW 新操作必须等待本人确认");
    await completedReply("完成 REVIEW");
    assert.notEqual(fixture.facts.reviewRequest, fixture.facts.request);
    const afterReview = await serviceCall("query");
    assert.equal(originalRequests(afterReview).size, 2);
    assert.equal((await task()).snapshot.attempts, 1);
    const pendingReview = taskPreparations(afterReview).find(
      (row) => originalRequest(row) === fixture.facts.reviewRequest,
    );
    assert.equal(pendingReview?.state, "awaitingConfirmation");
    assert.equal(pendingReview?.risk?.level, "two");
    assert.equal(pendingReview?.confirmation, null);
    assert.equal((await serviceCall("status")).startRequests, 1);
    result.checks.push("ai-risk-two-waits-for-human-without-start");
    await openNativeTask(
      afterReview.items.find(
        (item) =>
          item.reference.kind === "request" &&
          item.reference.request === fixture.facts.reviewRequest,
      ),
    );
    await text("原动作需要本人核对");
    // Opening/closing the original retained preparation is read-only.
    key(53);
    await wait(
      async () => !(await browser.$(".rss-task-detail").isDisplayed()),
    );
    assert.equal((await serviceCall("status")).startRequests, 1);
    result.checks.push("ai-original-preparation-read-without-start");

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
      await click("发现");
      await click("重读结果");
      await wait(() =>
        browser.$(".rss-store-grid .rss-software-card").isDisplayed(),
      );
      // Collapse the overlay before using the page companion on a narrow window.
      if (width < 900) {
        key(53);
        await wait(
          async () =>
            (await browser
              .$(".rss-desktop-shell")
              .getAttribute("data-layout-step")) === "0",
        );
      }
      await openPageAssistant();
      assert.equal(
        await browser.execute(
          () =>
            [...document.querySelectorAll(".rss-composer textarea")].filter(
              (node) => node.getClientRects().length > 0,
            ).length,
        ),
        1,
      );
      mark(`resource context ${width}×${height}: explicit provider context`);
      await browser
        .$(".rss-page-assistant textarea")
        .setValue("GOLDEN_CONTEXT 解释这个软件的版本与限制");
      assert.equal(
        await browser.execute(() => {
          const r = document
            .querySelector(".rss-page-assistant textarea")
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
      await click("关闭页面助手");
      await wait(() =>
        browser.execute(
          () =>
            document.activeElement?.getAttribute("aria-label") === "页面助手",
        ),
      );
      await navigate("AI 助手");
      assert.equal(
        await browser.$(".rss-assistant-input textarea").getValue(),
        "GOLDEN_CONTEXT 解释这个软件的版本与限制",
      );
    }
    await prompt("GOLDEN_CONTEXT 解释这个软件的版本与限制");
    await completedReply("完成 CONTEXT");
    assert.equal(fixture.facts.contexts.length, 1);
    assert.equal(
      fixture.facts.proposals.length,
      proposalsBeforeContext,
      "context inquiry does not execute tools",
    );
    result.checks.push(
      "page-context-explicit-attachment",
      "page-context-same-session",
      "page-context-provider-payload",
      "page-context-three-sizes",
    );
    mark(
      "explicit local collection and background history share the native task center",
    );
    const beforeCollection = originalRequests(await serviceCall("query"));
    const proposalsBeforeCollection = fixture.facts.proposals.length;
    await navigate("软件中心");
    await click("已安装");
    const collect = browser.$(
      '//button[normalize-space(.)="采集最新清单" and not(@disabled)]',
    );
    mark("explicit local collection: available native source");
    await wait(() => collect.isDisplayed());
    await collect.click();
    mark("explicit local collection: original query in unified task page");
    const collected = await wait(async () =>
      (await serviceCall("query")).items.find(
        (item) =>
          item.taskKind === "query" &&
          item.reference.kind === "request" &&
          !beforeCollection.has(item.reference.request),
      ),
    );
    mark("explicit local collection: open original detail");
    await openNativeTask(collected);
    mark("explicit local collection: Home activity entry");
    await navigate("主页");
    const recent = browser.$(
      `.rss-home-activity-row[data-task-reference=${JSON.stringify(JSON.stringify({ kind: "request", request: collected.reference.request }))}]`,
    );
    await wait(() => recent.isDisplayed());
    mark("explicit local collection: reopen original detail from Home");
    await recent.click();
    await wait(() => browser.$(".rss-task-detail").isDisplayed());
    mark("explicit local collection: return Home focus");
    await click(
      "关闭",
      '//*[contains(@class,"rss-task-detail") or contains(@class,"ant-drawer")]',
    );
    await wait(() =>
      browser.execute(
        (reference) =>
          document.activeElement?.getAttribute("data-task-reference") ===
          JSON.stringify(reference),
        collected.reference,
      ),
    );
    mark("background history: a collected periodic definition");
    const sampling = await wait(
      async () => sampledNativeTask(await serviceCall("query")),
      90000,
    );
    assert.equal(sampling.source, "policy");
    assert.deepEqual(sampling.allowedActions, []);
    await openNativeTask(sampling);
    assert.deepEqual(
      await Promise.all(
        (await browser.$$(".rss-task-drawer-actions button")).map((button) =>
          button.getText(),
        ),
      ),
      ["关闭", "查询原请求"],
    );
    mark("background history: management coverage and read-only detail");
    await navigate("任务中心");
    await click("管理活动", '//*[contains(@class,"rss-secondary-nav")]');
    const managementRef = {
      kind: "activity",
      id: serviceReady.history.activity,
    };
    await wait(() => taskButton(managementRef).isDisplayed());
    await taskButton(managementRef).click();
    await wait(() => browser.$(".rss-task-detail").isDisplayed());
    assert.ok(
      (await browser.$(".rss-task-coverage").getText()).includes("覆盖不完整"),
    );
    assert.ok(
      (await browser.$(".rss-task-detail").getText()).includes(
        "effect-not-observed",
      ),
    );
    assert.deepEqual(
      await Promise.all(
        (await browser.$$(".rss-task-drawer-actions button")).map((button) =>
          button.getText(),
        ),
      ),
      ["关闭", "查询原请求"],
    );
    const retainedRequests = originalRequests(await serviceCall("query"));
    await navigate("AI 助手");
    assert.deepEqual(
      originalRequests(await serviceCall("query")),
      retainedRequests,
    );
    assert.equal(fixture.facts.proposals.length, proposalsBeforeCollection);
    result.checks.push(
      "native-explicit-query-original-task",
      "native-home-task-focus",
      "native-background-sampling-read-only",
      "native-management-coverage-read-only",
      "task-navigation-without-ai-or-resubmit",
    );
    const readAppearance = () =>
      browser.execute(() => {
        const shell = document.querySelector(".rss-desktop-shell");
        return {
          materialEnabled: shell?.dataset.material === "true",
          reducedMotion: shell?.dataset.reducedMotion === "true",
          highContrast: shell?.dataset.highContrast === "true",
          bodyBackground: getComputedStyle(
            document.querySelector(".rss-shell-main"),
          ).backgroundColor,
        };
      });
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
          document.querySelector(".rss-desktop-shell")?.dataset.material ===
          String(expected),
        expectedMaterial,
      ),
    );
    result.checks.push("native-close-control-material-rebind");
    await wait(async () => !(await visibleText("正在读取或切换账户…")));
    await navigate("AI 助手");
    await wait(() => browser.$(".rss-composer textarea").isDisplayed());
    await text("GOLDEN_HELLO 查看办公套件");
    mark("restart Host through settings");
    rememberScopes(await readWorkerScopes(), hostPid());
    await openAgentSettings("运行与诊断");
    await click("重启 AI Host");
    await browser.$(".ant-popconfirm .ant-btn-primary").click();
    await wait(() => {
      const pid = hostPid();
      return pid && pid !== initialHostPid;
    });
    result.owner.restartedHostPid = hostPid();
    mark("reuse native credential after Host restart");
    await text("已就绪");
    await wait(
      async () =>
        await browser
          .$('//button[normalize-space(.)="重启 AI Host"]')
          .isEnabled(),
    );
    await openAgentSettings("AI 与连接");
    await click("配置 Golden Codex");
    await click("验证当前已保存配置");
    await text("已验证");
    mark("restore history and create a new provider session after restart");
    await navigate("AI 助手");
    await text("GOLDEN_HELLO 查看办公套件");
    await click("新会话");
    await prompt("GOLDEN_HELLO 重启后新对话");
    await completedReply("完成 HELLO");
    result.checks.push(
      "window-reopen-history",
      "host-restart-credential-reuse",
      "new-provider-session-after-restart",
    );
    if (security) {
      const runtime = {
        launcher: join(artifact, "bin/rss-ai-worker-launcher"),
        manifestDigest: sha256(
          readFileSync(join(artifact, "worker-manifest.json")),
        ),
      };
      const fences = readWorkerScopes;
      const absent = (scope) =>
        scopeAbsentWithin(runtime, scope, {
          timeoutMs: 1000,
          signal: new AbortController().signal,
        });
      const proof = {
        runtimeTreeSha256: manifest.runtimeTreeSha256,
        scenarios: {},
      };
      result.processSecurity = proof;
      const restart = async () => {
        const previous = hostPid();
        rememberScopes(await fences(), previous);
        await openAgentSettings("运行与诊断");
        await click("重启 AI Host");
        await browser.$(".ant-popconfirm .ant-btn-primary").click();
        await wait(() => hostPid() && hostPid() !== previous);
        if (previous)
          await wait(() => {
            try {
              process.kill(previous, 0);
              return false;
            } catch (error) {
              if (error.code === "ESRCH") return true;
              throw error;
            }
          });
        await text("已就绪");
        result.owner.restartedHostPid = hostPid();
      };
      const held = async () => {
        const existing = await fences();
        rememberScopes(existing, hostPid());
        const previousRoots = new Set(existing.map((row) => row.scope.root));
        await navigate("AI 助手");
        await click("新会话");
        const previous = fixture.facts.held;
        await prompt("GOLDEN_HOLD 安全验收：保持请求等待");
        await wait(() => fixture.facts.held > previous);
        const observed = await fences();
        rememberScopes(observed, hostPid());
        const rows = observed.filter(
          (row) => !previousRoots.has(row.scope.root),
        );
        assert.equal(rows.length, 1, "one actual live Codex worker scope");
        assert.equal(rows[0].scope.kind, "processGroup");
        assert.equal(await absent(rows[0].scope), false);
        rememberScopes(rows, hostPid());
        assert.ok(
          ownedWorkerScopes.get(rows[0].scope.root)?.anchor,
          "live native worker ownership required before fault injection",
        );
        return rows[0];
      };
      const record = async (name, evidence) => {
        await serviceCall("processSecurity", {
          proof: {
            runtimeTreeSha256: proof.runtimeTreeSha256,
            scenarios: { [name]: evidence },
          },
        });
        proof.scenarios[name] = evidence;
        result.checks.push(name);
      };
      mark("security: unresponsive actual launcher, bounded Host close");
      await serviceCall("processSecurityBegin", { name: "close_timeout" });
      let launch = await held();
      const closeStart = performance.now();
      process.kill(launch.scope.root, "SIGSTOP");
      await restart();
      await wait(() => absent(launch.scope));
      await record("close_timeout", {
        launch,
        stoppedLauncher: true,
        elapsedMs: performance.now() - closeStart,
        scopeAbsent: true,
      });

      mark(
        "security: kill launcher, retain descendant and block unknown restore",
      );
      await serviceCall("processSecurityBegin", { name: "launcher_crash" });
      launch = await held();
      const requests = fixture.facts.requests;
      const survivingHost = hostPid();
      assert.equal(Number(processIdentity(survivingHost, "ppid")), receipt.pid);
      process.kill(launch.scope.root, "SIGKILL");
      await wait(() => absent(launch.scope));
      assert.ok(
        processIdentity(survivingHost, "comm"),
        "Host must survive launcher failure",
      );
      assert.equal(hostPid(), survivingHost);
      await record("launcher_crash", {
        launch,
        survivingHost,
        scopeAbsent: true,
      });
      await serviceCall("processSecurityBegin", {
        name: "retained_descendant",
      });
      await serviceCall("processSecurityBegin", { name: "unknown_scope" });
      faultScopeConfirmed = false;
      const recovery = await verifyUnknownFence();
      faultScopeConfirmed = recovery.scopeAbsentAfterOwnedCleanup === true;
      assert.equal(
        fixture.facts.requests,
        requests,
        "fault recovery cannot redispatch the model request",
      );
      await record("retained_descendant", recovery);
      await record("unknown_scope", recovery);
      await restart();

      mark("security: actual Host crash and owned worker group termination");
      await serviceCall("processSecurityBegin", { name: "host_crash" });
      launch = await held();
      const crashedHost = hostPid();
      assert.equal(Number(processIdentity(crashedHost, "ppid")), receipt.pid);
      process.kill(crashedHost, "SIGKILL");
      await wait(() => absent(launch.scope));
      await restart();
      await record("host_crash", {
        hostPid: crashedHost,
        launch,
        scopeAbsent: true,
      });

      result.processSecurity = proof;
    }
    mark(
      "credential deletion through the current subject's saved configuration",
    );
    await openAgentSettings("AI 与连接");
    await click("配置 Golden Codex");
    await click("移除连接");
    await browser.$(".ant-popconfirm .ant-btn-primary").click();
    await text("还没有 AI 连接");
    assert.equal(
      (await browser.getPageSource()).includes(fixture.secret),
      false,
    );
    assert.equal(logs.includes(fixture.secret), false);
    result.checks.push("credential-deletion", "secret-redaction");
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
    assert.ok(security ? sessions.length >= 2 : sessions.length === 2);
    assert.equal(
      new Set(sessions.map((session) => session.namespace.principalId)).size,
      1,
    );
    assert.equal(sessions[0].stages[0].binding.providerVersion, "0.155.0");
    assert.equal((await task()).snapshot.attempts, 1);
    result.completion = await serviceCall("completion", {
      request: fixture.facts.request,
    });
    assert.equal(result.completion.request, fixture.facts.request);
    assert.equal(result.completion.record.status.process.finished, true);
    result.backend = result.completion.backend;
    result.deviceEffect = result.completion.effect;
    result.checks.push(
      "service-terminal-process",
      "backend-result-acknowledged",
      "independent-device-effect",
    );
    result.facts = {
      sessions: sessions.length,
      attempts: (await task()).snapshot.attempts,
      model: fixture.facts,
    };
    result.checks.push("native-menu-quit", "host-processes-reaped");
    if (!candidatePath)
      assert.deepEqual(
        sourceEvidence(root),
        result.developmentSource,
        "source changed during acceptance",
      );
    assert.equal(
      sha256(readFileSync(join(artifact, "manifest.json"))),
      result.runtimeManifestSha256,
    );
    verifyRuntimeIntegrity(artifact, manifest.runtimeTreeSha256);
    if (candidatePath) verifyCandidate(candidatePath);
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
      result.uiFailure = await browser.execute(() => ({
        focusedTag: document.activeElement?.tagName,
        focusedAction: document.activeElement?.getAttribute("data-action"),
        focusedLabel: document.activeElement?.getAttribute("aria-label"),
        focusedClass: document.activeElement?.className,
        dialogLabels: [...document.querySelectorAll('[role="dialog"]')]
          .filter((el) => el.getClientRects().length > 0)
          .map(
            (el) =>
              el.getAttribute("aria-label") ||
              el.getAttribute("aria-labelledby"),
          ),
        navigation: [
          ...document.querySelectorAll(".rss-primary-nav button"),
        ].map((el) => ({
          text: el.textContent,
          visible: !!el.getClientRects().length,
          inert: !!el.closest("[inert]"),
          disabled: el.disabled,
        })),
        viewport: { width: innerWidth, height: innerHeight },
        compactTriggerPresent: !!document.querySelector(
          '[aria-label="切换侧栏布局"]',
        ),
      }));
      result.transportFailure = await browser.execute(() => ({
        display: document.querySelector(".assistant")?.style.display,
        state:
          document.querySelector('[role="alert"]')?.textContent ??
          "unavailable",
      }));
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
  let scopeObservationFailed = false;
  if (scopeRuntime && directory && existsSync(join(directory, "ai.sqlite"))) {
    try {
      rememberScopes(
        await readWorkerScopes(),
        result.owner?.restartedHostPid ?? result.owner?.hostPid,
      );
    } catch {
      scopeObservationFailed = true;
    }
  }
  awake?.kill("SIGTERM");
  if (browser) {
    browserClosing ??= browser.deleteSession().catch(() => {});
    await Promise.race([browserClosing, delay(5000)]);
  }
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
  const scopeCleanup = [];
  for (const entry of ownedWorkerScopes.values()) {
    const absent = () =>
      scopeAbsentWithin(scopeRuntime, entry.scope, {
        timeoutMs: 1000,
        signal: new AbortController().signal,
      });
    let confirmed = false;
    try {
      confirmed = await absent();
      if (!confirmed && terminateObservedScope(entry)) {
        const deadline = Date.now() + 7000;
        do {
          confirmed = await absent();
          if (confirmed) break;
          await delay(100);
        } while (Date.now() < deadline);
      }
    } catch {
      scopeObservationFailed = true;
    }
    scopeCleanup.push({
      scope: entry.scope,
      launchId: entry.launchId,
      confirmed,
      ownershipObserved: !!entry.anchor,
    });
  }
  result.processScopeProof = {
    runtimeTreeSha256: scopeRuntime?.runtimeTreeSha256,
    observationComplete: !scopeObservationFailed && faultScopeConfirmed,
    scopes: scopeCleanup,
  };
  const cleanupComplete =
    alive().length === 0 &&
    !rootAlive() &&
    !scopeObservationFailed &&
    faultScopeConfirmed &&
    scopeCleanup.every((row) => row.confirmed);
  if (!cleanupComplete) {
    result.cleanup = "owned-processes-still-present";
    result.status = cancelled ? "cancelled" : "failed";
    process.exitCode = 1;
  }
  let keychainClean = cleanupComplete;
  if (!visualOnly && directory && cleanupComplete) {
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
  if (serviceWorker) {
    if (
      serviceReady &&
      serviceWorker.exitCode === null &&
      serviceWorker.signalCode === null
    ) {
      try {
        serviceFinishing = true;
        await serviceCall("finish", {
          status: result.status,
          request: fixture?.facts.request,
          processScopeEmpty: cleanupComplete,
          processScopeProof: result.processScopeProof,
        });
      } catch {
        result.status = "failed";
        process.exitCode = 1;
      }
    }
    console.log("[native] bounded controlled service owner cleanup");
    serviceWorker.stdin.end();
    result.serviceWorkerCleanup = await reapOwnedProcessGroup(serviceWorker);
    if (
      !result.serviceWorkerCleanup.confirmed ||
      result.serviceWorkerCleanup.forced
    ) {
      result.status = cancelled ? "cancelled" : "failed";
      result.serviceFailure = {
        stage,
        code: "native_service_worker_cleanup_forced",
        detail: "controlled service worker required bounded termination",
      };
      result.failure ??= result.serviceFailure;
      process.exitCode = 1;
    }
    const serviceReceipt =
      result.serviceEvidence && join(result.serviceEvidence, "receipt.json");
    if (serviceReceipt && existsSync(serviceReceipt))
      result.serviceReceipt = JSON.parse(readFileSync(serviceReceipt, "utf8"));
    result.serviceCleanup =
      result.serviceReceipt?.cleanup === "complete"
        ? result.serviceReceipt.installationCreated
          ? "owned registration and default pin removed; persistent service state retained"
          : "no owned system registration created; preparation resources cleaned"
        : "unconfirmed; retained installation evidence";
    if (serviceWorker.exitCode !== 0) {
      result.status =
        result.serviceReceipt?.status === "cancelled" || cancelled
          ? "cancelled"
          : "failed";
      if (result.serviceReceipt?.error)
        result.serviceFailure = {
          stage,
          code: "native_service_journey_incomplete",
          detail: result.serviceReceipt.error,
        };
      result.failure ??= result.serviceFailure;
      process.exitCode = 1;
    } else {
      if (!visualOnly && result.status === "passed") {
        const records = result.serviceReceipt.journalProof.records.filter(
          (row) => row.request === fixture.facts.request,
        );
        assert.equal(records.length, 1);
        assert.equal(records[0].snapshot.attempts, 1);
        assert.equal(records[0].process.finished, true);
        assert.equal(
          records[0].process.attemptId,
          result.completion.record.status.attemptId,
        );
        assert.equal(
          records[0].process.exitCode,
          result.completion.record.status.process.exitCode,
        );
        assert.equal(result.serviceReceipt.journeyStatus, "passed");
      }
    }
  }
  result.journeyStatus = result.status;
  if (
    result.status === "passed" &&
    result.serviceReceipt?.security.status !== "passed" &&
    controlledService
  ) {
    result.status = "partial";
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
