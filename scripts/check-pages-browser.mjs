import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
export async function checkPagesBrowser(page, fixture) {
  const directory = new URL("../.local-ci-runs/", import.meta.url);
  mkdirSync(directory, { recursive: true });
  const evidence = {
    assembly: "explicit UI fixtures in production React pages",
    platformProof: false,
    captures: [],
    checks: [],
  };
  const go = async (scene = "normal") => {
    const reset = await fetch(new URL("/__fixture/scenario", fixture.url), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ scenario: "running" }),
    });
    assert.equal(reset.ok, true);
    await page.goto(`${fixture.url}?desktop&scene=${scene}`);
    await page
      .getByRole("button", { name: "切换侧栏布局", exact: true })
      .waitFor();
    await page.locator(".fixture-controls").evaluate((node) => {
      node.style.display = "none";
    });
  };
  const primary = (name) =>
    page
      .locator(".rss-hidden-primary")
      .getByRole("button", { name, exact: true })
      .click();
  const section = (name) =>
    page
      .locator(".rss-hidden-secondary")
      .getByRole("button", { name, exact: true })
      .click();
  const settings = async (name) => {
    await page
      .getByRole("button", { name: "RSS MDM Agent 设置", exact: true })
      .click();
    await page.getByRole("menuitem", { name, exact: true }).click();
  };
  const capture = async (name) => {
    await page.evaluate(async () => {
      document
        .querySelector(".rss-page-viewport:not([hidden])")
        ?.scrollTo({ top: 0 });
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
    });
    const file = `react-pages-${name}.png`;
    await page.screenshot({
      path: new URL(file, directory).pathname,
      animations: "disabled",
    });
    const storyContrast = await page
      .locator(".rss-page-viewport:not([hidden]) .rss-store-editorial h2")
      .evaluateAll((nodes) =>
        nodes.map((node) => {
          const channels = (value) =>
            (value.match(/[\d.]+/g) ?? [])
              .slice(0, 3)
              .map(Number)
              .map((v) => {
                const n = v / 255;
                return n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4;
              });
          const luminance = (value) => {
            const rgb = channels(value);
            return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
          };
          const foreground = luminance(getComputedStyle(node).color),
            background = luminance(
              getComputedStyle(node.closest("section")).backgroundColor,
            );
          return (
            (Math.max(foreground, background) + 0.05) /
            (Math.min(foreground, background) + 0.05)
          );
        }),
      );
    assert.ok(
      storyContrast.every((ratio) => ratio >= 3),
      `${name}: editorial heading contrast`,
    );
    const dimensions = await page.evaluate(() => ({
      scroll: document.documentElement.scrollWidth,
      viewport: innerWidth,
      header: document.querySelector(".rss-topbar").getBoundingClientRect()
        .height,
    }));
    assert.equal(dimensions.header, 54);
    assert.ok(
      dimensions.scroll <= dimensions.viewport,
      `${name}: horizontal overflow`,
    );
    const grids = await page
      .locator(
        ".rss-page-viewport:not([hidden]) .rss-system-cards, .rss-page-viewport:not([hidden]) .rss-tool-grid",
      )
      .evaluateAll((nodes) =>
        nodes
          .filter((node) => node.getClientRects().length)
          .map(
            (node) =>
              getComputedStyle(node).gridTemplateColumns.split(/\s+/).length,
          ),
      );
    assert.ok(
      grids.every((columns) => columns === (dimensions.viewport > 420 ? 2 : 1)),
      `${name}: approved system/tools column count`,
    );
    evidence.captures.push({ file, ...dimensions });
  };
  await page.setViewportSize({ width: 1280, height: 900 });
  await go();
  await primary("软件中心");
  await page.getByRole("button", { name: "工作空间", exact: true }).waitFor();
  assert.equal(await page.locator(".rss-store-editorial > section").count(), 2);
  assert.equal(
    await page.evaluate(() => window.rssPageFixture.events.length),
    0,
  );
  await capture("discover");
  for (const category of [
    "效率办公",
    "创意设计",
    "网络工具",
    "开发工具",
    "系统工具",
  ]) {
    await section(category);
    await page.getByRole("heading", { name: category, exact: true }).waitFor();
    assert.equal(
      await page.locator(".rss-store-grid .rss-software-card").count(),
      1,
    );
  }
  await section("创意设计");
  await page.getByRole("button", { name: "创作画板", exact: true }).click();
  await page.getByRole("heading", { name: "创作画板", exact: true }).waitFor();
  await capture("detail-overview");
  for (const tab of ["版本记录", "权限与范围", "相关任务"]) {
    await page.getByRole("tab", { name: tab, exact: true }).click();
    await capture(`detail-${tab}`);
  }
  await page.getByRole("button", { name: "查看并准备", exact: true }).click();
  const publishedPreparation = page.getByRole("dialog");
  await publishedPreparation
    .getByRole("button", { name: "确认并提交", exact: true })
    .click();
  await publishedPreparation
    .getByRole("button", { name: "查询原请求", exact: true })
    .waitFor();
  await page.keyboard.press("Escape");
  await publishedPreparation.waitFor({ state: "hidden" });
  await page.getByRole("button", { name: "重读发布任务", exact: true }).click();
  await page
    .locator(".rss-related-task")
    .filter({ hasText: "创作画板" })
    .waitFor();
  await section("已安装");
  await page.getByRole("button", { name: "Workspace", exact: true }).waitFor();
  const first = page.locator(".rss-installed-card").first();
  assert.equal(
    await first.getByRole("button", { name: "卸载", exact: true }).isDisabled(),
    true,
  );
  assert.match(await first.textContent(), /由组织保护/);
  await capture("installed-cards");
  await page
    .locator(".ant-segmented")
    .getByText("列表", { exact: true })
    .click();
  await page
    .getByRole("columnheader", { name: "安装版本", exact: true })
    .waitFor();
  await capture("installed-list");
  await page
    .locator(".ant-segmented")
    .getByText("卡片", { exact: true })
    .click();
  const notes = page
    .locator(".rss-installed-card")
    .filter({ has: page.getByRole("button", { name: "Notes", exact: true }) });
  await notes.getByRole("button", { name: "卸载", exact: true }).click();
  const preparation = page.getByRole("dialog");
  await preparation
    .getByRole("button", { name: "确认卸载", exact: true })
    .waitFor();
  assert.equal(
    await page.evaluate(() =>
      window.rssPageFixture.events.some(
        (event) => event.operation === "software_submit",
      ),
    ),
    false,
  );
  await capture("software-preparation");
  await preparation
    .getByRole("button", { name: "确认卸载", exact: true })
    .click();
  await page.waitForFunction(() =>
    window.rssPageFixture.events.some(
      (event) => event.operation === "software_submit",
    ),
  );
  const requests = await page.evaluate(() =>
    window.rssPageFixture.events
      .filter((event) => event.operation.startsWith("software_"))
      .map((event) => event.request),
  );
  assert.equal(new Set(requests).size, 1);
  await page.keyboard.press("Escape");
  await preparation.waitFor({ state: "hidden" });
  await section("更新");
  assert.equal(await page.locator(".rss-installed-card").count(), 1);
  await capture("updates");
  evidence.checks.push(
    "all categories; full detail tabs; protected uninstall; cards/list; exact original preparation and explicit submit",
  );

  await primary("系统设置");
  await page
    .getByRole("heading", { name: "Studio Mac", exact: true })
    .waitFor();
  await capture("system-overview");
  for (const title of [
    "网络",
    "显示器",
    "声音",
    "电池与电源",
    "存储空间",
    "隐私与安全性",
    "关于这台电脑",
  ]) {
    await section(title);
    await page.getByRole("heading", { name: title, exact: true }).waitFor();
    await capture(`system-${title}`);
  }
  await section("网络");
  await page
    .getByRole("textbox", { name: "HTTP 代理服务器", exact: true })
    .fill("proxy.fixture.invalid");
  await section("显示器");
  await section("网络");
  assert.equal(
    await page
      .getByRole("textbox", { name: "HTTP 代理服务器", exact: true })
      .inputValue(),
    "proxy.fixture.invalid",
  );
  await page.getByRole("button", { name: "保存更改", exact: true }).click();
  await page.waitForFunction(() =>
    window.rssPageFixture.events.some(
      (event) => event.operation === "settings_change",
    ),
  );
  await page.waitForFunction(
    () =>
      !window.rssDesktop.state.workspace.system.state.pending.has("network"),
  );
  await page
    .getByRole("textbox", { name: "HTTP 代理服务器", exact: true })
    .fill("proxy.second.invalid");
  await page.getByRole("button", { name: "保存更改", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.rssPageFixture.events.filter(
        (event) => event.operation === "settings_change",
      ).length === 2,
  );
  await page.waitForFunction(
    () =>
      !window.rssDesktop.state.workspace.system.state.pending.has("network"),
  );
  const bypassInput = page.getByRole("textbox", {
    name: "Wi-Fi 绕过代理的地址",
    exact: true,
  });
  const bypassRevision = await page.evaluate(
    () =>
      window.rssDesktop.state.workspace.system.state.observations.get("network")
        .snapshot.revision,
  );
  await bypassInput.fill(" localhost\n *.corp.fixture.invalid \n");
  await page.getByRole("button", { name: "保存更改", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.rssPageFixture.events.filter(
        (event) => event.operation === "settings_change",
      ).length === 3 &&
      !window.rssDesktop.state.workspace.system.state.pending.has("network"),
  );
  const bypassChange = await page.evaluate(() =>
    window.rssPageFixture.events
      .filter((event) => event.operation === "settings_change")
      .at(-1),
  );
  assert.deepEqual(bypassChange.patch, {
    kind: "proxy",
    target: "wifi",
    http: null,
    https: null,
    socks: null,
    bypass: ["localhost", "*.corp.fixture.invalid"],
  });
  assert.equal(bypassChange.expected, bypassRevision);
  await page
    .locator(".rss-system-editor:visible")
    .getByText("修改仅在保存后提交", { exact: true })
    .waitFor();
  await bypassInput.fill("");
  await page.getByRole("button", { name: "保存更改", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.rssPageFixture.events.filter(
        (event) => event.operation === "settings_change",
      ).length === 4 &&
      !window.rssDesktop.state.workspace.system.state.pending.has("network"),
  );
  assert.deepEqual(
    await page.evaluate(
      () =>
        window.rssPageFixture.events
          .filter((event) => event.operation === "settings_change")
          .at(-1).patch.bypass,
    ),
    [],
  );
  await capture("proxy-bypass-cleared");
  await page.setViewportSize({ width: 420, height: 900 });
  await bypassInput.scrollIntoViewIfNeeded();
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  );
  await page.screenshot({
    path: new URL("react-pages-proxy-bypass-narrow.png", directory).pathname,
    animations: "disabled",
  });
  evidence.captures.push({
    file: "react-pages-proxy-bypass-narrow.png",
    width: 420,
    state: "bypass-cleared",
  });
  await page.setViewportSize({ width: 1280, height: 900 });
  await section("显示器");
  const displayMode = page.getByRole("combobox", {
    name: "内建显示器 显示模式",
    exact: true,
  });
  await displayMode.focus();
  await displayMode.press("ArrowDown");
  await displayMode.press("ArrowDown");
  await displayMode.press("Enter");
  const displayRevision = await page.evaluate(
    () =>
      window.rssDesktop.state.workspace.system.state.observations.get("display")
        .snapshot.revision,
  );
  await page
    .getByRole("spinbutton", { name: "内建显示器 桌面横坐标", exact: true })
    .fill("-1920");
  await page
    .getByRole("spinbutton", { name: "内建显示器 桌面纵坐标", exact: true })
    .fill("80");
  await page.getByRole("button", { name: "保存更改", exact: true }).click();
  const keepDisplay = page.getByRole("button", {
    name: "保留显示设置",
    exact: true,
  });
  await keepDisplay.waitFor();
  assert.equal(await keepDisplay.isEnabled(), true);
  const displayChange = await page.evaluate(() =>
    window.rssPageFixture.events
      .filter((event) => event.operation === "settings_change")
      .at(-1),
  );
  assert.deepEqual(displayChange.patch, {
    kind: "display",
    target: "internal",
    mode: "large",
    origin: { x: -1920, y: 80 },
  });
  assert.equal(displayChange.expected, displayRevision);
  assert.equal(
    await page
      .getByText(
        "观察版本已变化；保存仍核对开始编辑时的版本。请先重新核对草稿。",
        { exact: true },
      )
      .count(),
    0,
  );
  await capture("display-preview");
  await page.setViewportSize({ width: 420, height: 900 });
  await page
    .getByRole("spinbutton", { name: "内建显示器 桌面纵坐标", exact: true })
    .scrollIntoViewIfNeeded();
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
  );
  await page.screenshot({
    path: new URL("react-pages-display-origin-narrow.png", directory).pathname,
    animations: "disabled",
  });
  evidence.captures.push({
    file: "react-pages-display-origin-narrow.png",
    width: 420,
    state: "display-preview",
  });
  await page.setViewportSize({ width: 1280, height: 900 });
  await keepDisplay.click();
  await page.waitForFunction(
    () =>
      !window.rssDesktop.state.workspace.system.state.pending.has("display"),
  );
  assert.equal(await keepDisplay.count(), 0);
  evidence.checks.push(
    "consecutive settings saves use changed CAS revisions; bypass edit and explicit empty clear send only exact deltas; signed display coordinates and chosen mode use the original revision, with preview and explicit keep before deadline",
  );
  await primary("工具中心");
  await section("连接与网络");
  await page.getByRole("button", { name: /开始检查/ }).click();
  const network = page.getByRole("dialog");
  await network
    .getByRole("textbox", { name: "网络检测主机", exact: true })
    .waitFor();
  assert.equal(
    await page.evaluate(() =>
      window.rssPageFixture.events.some(
        (event) => event.operation === "network_run",
      ),
    ),
    false,
  );
  await network.getByRole("button", { name: "开始检测", exact: true }).click();
  await network
    .getByRole("heading", { name: "实际检测结果", exact: true })
    .waitFor();
  await capture("network-result");
  await page.keyboard.press("Escape");
  await network.waitFor({ state: "hidden" });
  await section("全部工具");
  await page
    .getByRole("heading", { name: "组织网络观察", exact: true })
    .waitFor();
  await page
    .locator(".rss-tool-card")
    .filter({
      has: page.getByRole("heading", { name: "组织网络观察", exact: true }),
    })
    .getByRole("button", { name: /查看并准备/ })
    .click();
  const policy = page.getByRole("dialog");
  await policy
    .getByRole("textbox", { name: "interface", exact: true })
    .fill("en0");
  assert.equal(
    await page.evaluate(
      () =>
        window.rssPageFixture.events.filter(
          (event) => event.operation === "policy_invoke",
        ).length,
    ),
    1,
  );
  await policy.getByRole("button", { name: "确认并提交", exact: true }).click();
  await policy
    .getByRole("button", { name: "查询原请求", exact: true })
    .waitFor();
  await capture("published-tool");
  await page.keyboard.press("Escape");
  await policy.waitFor({ state: "hidden" });
  await page.evaluate(() => {
    const request =
      window.rssDesktop.state.workspace.policies.state.submission.choice
        .request;
    window.rssPageFixture.records.get(request).state = "failed";
  });
  await page
    .locator(".rss-tool-card")
    .filter({
      has: page.getByRole("heading", { name: "组织网络观察", exact: true }),
    })
    .getByRole("button", { name: /查看并准备/ })
    .click();
  await policy.getByRole("button", { name: "查询原请求", exact: true }).click();
  await policy.getByRole("button", { name: "重新准备", exact: true }).click();
  const freshInput = policy.getByRole("textbox", {
    name: "interface",
    exact: true,
  });
  await freshInput.waitFor();
  assert.equal(await freshInput.inputValue(), "");
  await freshInput.fill("en1");
  const beforeRestart = await page.evaluate(
    () => window.rssDesktop.state.workspace.policies.state.submission,
  );
  assert.equal(beforeRestart, null);
  await policy.getByRole("button", { name: "确认并提交", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.rssPageFixture.events.filter(
        (event) => event.operation === "policy_invoke",
      ).length === 3,
  );
  const invocations = await page.evaluate(() =>
    window.rssPageFixture.events
      .filter((event) => event.operation === "policy_invoke")
      .map((event) => event.request),
  );
  assert.equal(new Set(invocations).size, 3);
  await page.keyboard.press("Escape");
  await policy.waitFor({ state: "hidden" });
  evidence.checks.push(
    "same organization project explicitly re-resolves after failure; parameters reset; a new human submission gets a distinct request",
  );
  evidence.checks.push(
    "eight system sections; frozen proxy draft; four tool groups; explicit DNS/TCP and published script invocation",
  );

  await settings("AI 与连接");
  await page.getByRole("button", { name: "添加连接", exact: true }).click();
  const connection = page.getByRole("dialog");
  await connection
    .getByRole("textbox", { name: "连接名称", exact: true })
    .fill("工作 API");
  await connection
    .getByRole("combobox", { name: "AI 配置来源", exact: true })
    .click();
  await connection
    .getByRole("combobox", { name: "AI 配置来源", exact: true })
    .press("ArrowDown");
  await connection
    .getByRole("combobox", { name: "AI 配置来源", exact: true })
    .press("Enter");
  await connection
    .getByRole("textbox", { name: "AI API 地址", exact: true })
    .fill("https://api.fixture.invalid");
  await connection
    .getByRole("textbox", { name: "AI 模型", exact: true })
    .fill("fixed-model");
  await connection
    .locator('input[aria-label="AI 新密钥"]')
    .fill("ui-secret-proof");
  await page.evaluate(() =>
    window.rssDesktop.state.workspace.navigation.select("home"),
  );
  await connection.waitFor({ state: "hidden" });
  await settings("AI 与连接");
  await connection.waitFor();
  assert.equal(
    await connection
      .getByRole("textbox", { name: "连接名称", exact: true })
      .inputValue(),
    "工作 API",
  );
  assert.equal(
    await connection.locator('input[aria-label="AI 新密钥"]').inputValue(),
    "",
  );
  await connection
    .locator('input[aria-label="AI 新密钥"]')
    .fill("ui-secret-proof");
  await connection
    .getByRole("textbox", { name: "AI API 地址", exact: true })
    .fill("https://other.fixture.invalid");
  assert.equal(
    await connection.locator('input[aria-label="AI 新密钥"]').inputValue(),
    "",
  );
  await connection
    .locator('input[aria-label="AI 新密钥"]')
    .fill("ui-secret-proof");
  await connection
    .getByRole("button", { name: "保存连接", exact: true })
    .click();
  await connection.waitFor({ state: "hidden" });
  await page.getByRole("heading", { name: "工作 API", exact: true }).waitFor();
  assert.equal(
    await page.evaluate(() =>
      JSON.stringify(
        window.rssDesktop.state.workspace.connections.state,
      ).includes("ui-secret-proof"),
    ),
    false,
  );
  await capture("connections");
  await settings("个人 Passkey");
  await page
    .getByRole("button", { name: "通过系统验证解锁", exact: true })
    .click();
  await page.getByText("work.fixture.invalid", { exact: true }).waitFor();
  assert.equal(
    await page
      .locator(
        '.rss-page-viewport[aria-label="RSS MDM Agent 设置"] input[type="password"]',
      )
      .count(),
    0,
  );
  await capture("personal-passkey");
  await settings("运行与诊断");
  await page.getByRole("button", { name: "预览脱敏诊断", exact: true }).click();
  const diagnostic = page.getByRole("dialog");
  await diagnostic.getByText(/"credentials": "redacted"/).waitFor();
  await capture("diagnostics-preview");
  await diagnostic
    .getByRole("button", { name: "保存此脱敏快照", exact: true })
    .click();
  await diagnostic.getByText("已取消导出。", { exact: true }).waitFor();
  await page.keyboard.press("Escape");
  await diagnostic.waitFor({ state: "hidden" });
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.diagnostics.state.preview == null,
    ),
    true,
  );
  await settings("关于与隐私");
  await capture("about");
  evidence.checks.push(
    "native-only secret save boundary UI, endpoint secret clear, Passkey metadata, redacted fixed snapshot and export cancellation/discard",
  );

  for (const theme of ["light", "dark"])
    for (const width of [1280, 720, 420]) {
      await page.setViewportSize({ width, height: 900 });
      await page.evaluate((mode) => window.rssDesktop.setTheme(mode), theme);
      for (const [module, child, name] of [
        ["软件中心", "发现", "software"],
        ["系统设置", "概览", "system"],
        ["工具中心", "全部工具", "tools"],
      ]) {
        await primary(module);
        await section(child);
        await capture(`${name}-${theme}-${width}`);
      }
      await settings("AI 与连接");
      await capture(`settings-${theme}-${width}`);
    }
  await page.setViewportSize({ width: 1280, height: 900 });
  await go("loading");
  await primary("软件中心");
  const loadingPage = page.locator('.rss-page-viewport[aria-label="软件中心"]');
  const pendingDirectory = loadingPage.getByRole("status", {
    name: "正在读取发布目录",
    exact: true,
  });
  await pendingDirectory.waitFor();
  assert.equal(await pendingDirectory.getAttribute("aria-busy"), "true");
  assert.equal(
    await loadingPage.getByRole("button", { name: /重读结果$/ }).isDisabled(),
    true,
  );
  assert.equal(await loadingPage.locator(".rss-software-card").count(), 0);
  assert.equal(
    await page.evaluate(() => window.rssPageFixture.events.length),
    0,
  );
  await capture("directory-loading");
  await page.evaluate(() => window.rssPageFixture.releaseDirectory());
  await loadingPage
    .getByRole("button", { name: "工作空间", exact: true })
    .waitFor();
  assert.equal(
    await loadingPage.getByRole("button", { name: /重读结果$/ }).isEnabled(),
    true,
  );
  evidence.checks.push(
    "observable pending directory read disables duplicate refresh; releasing the same read renders results without executing",
  );

  for (const scene of ["empty", "partial", "error", "offline", "readonly"]) {
    await page.setViewportSize({ width: 1280, height: 900 });
    await go(scene);
    await primary("软件中心");
    await section("已安装");
    await page.waitForFunction(
      () =>
        !window.rssDesktop.state.workspace.software.state.loading &&
        !window.rssDesktop.state.workspace.software.state.sources.some(
          (source) => source.busy,
        ),
    );
    const softwarePage = page.locator(
      '.rss-page-viewport[aria-label="软件中心"]',
    );
    if (scene === "empty") {
      await softwarePage
        .getByText("当前范围没有已读取的软件", { exact: true })
        .waitFor();
      assert.equal(
        await softwarePage.locator(".rss-installed-card").count(),
        0,
      );
    }
    if (scene === "partial")
      assert.match(
        await softwarePage.locator(".rss-software-source").textContent(),
        /部分或尚未读取/,
      );
    if (["error", "offline"].includes(scene)) {
      await softwarePage
        .locator('[role="alert"]')
        .filter({ hasText: "清单读取未完成" })
        .waitFor();
      assert.equal(
        await softwarePage
          .getByRole("button", { name: "Workspace", exact: true })
          .count(),
        0,
      );
    }
    if (scene === "readonly") {
      const buttons = softwarePage.getByRole("button", {
        name: "打开",
        exact: true,
      });
      assert.equal(await buttons.count(), 4);
      for (const button of await buttons.all())
        assert.equal(await button.isDisabled(), true);
    }
    await capture(`software-${scene}`);
    await primary("系统设置");
    await section("网络");
    await page.waitForFunction(() => {
      const owner = window.rssDesktop.state.workspace.system;
      return (
        !owner.state.busy.has("network") && !owner.state.reads.has("network")
      );
    });
    const systemPage = page.locator(
      '.rss-page-viewport[aria-label="系统设置"]',
    );
    if (["error", "offline"].includes(scene))
      await systemPage
        .getByRole("alert")
        .filter({ hasText: "原任务" })
        .waitFor();
    if (scene === "readonly") {
      assert.equal(
        await systemPage
          .getByRole("button", { name: "保存更改", exact: true })
          .count(),
        0,
      );
      await systemPage.getByText("此用户仅可读取", { exact: true }).waitFor();
    }
    if (scene === "empty")
      assert.equal(await systemPage.locator(".rss-system-editor").count(), 0);
    await capture(`system-${scene}`);
    await primary("工具中心");
    await section("全部工具");
    if (scene === "empty")
      await page
        .getByText("当前读取范围没有可见组织工具", { exact: true })
        .waitFor();
    if (["error", "offline"].includes(scene))
      await page
        .locator('.rss-published-tools [role="alert"]')
        .filter({ hasText: "目录读取未完成" })
        .waitFor();
    await section("连接与网络");
    await page.getByRole("button", { name: /开始检查/ }).click();
    const check = page.getByRole("dialog");
    if (scene === "readonly") {
      await check
        .getByText("当前装配未提供原生网络检测能力。", { exact: true })
        .waitFor();
      assert.equal(
        await check
          .getByRole("button", { name: "开始检测", exact: true })
          .isDisabled(),
        true,
      );
    }
    if (["error", "offline"].includes(scene))
      await check
        .getByText("无法读取默认检测目标，可手动填写主机与端口。", {
          exact: true,
        })
        .waitFor();
    await page.keyboard.press("Escape");
    await check.waitFor({ state: "hidden" });
    await settings("AI 与连接");
    if (["error", "offline"].includes(scene)) {
      await page.getByText("AI 控制连接未就绪", { exact: true }).waitFor();
      assert.equal(
        await page
          .getByRole("button", { name: "重读连接", exact: true })
          .isDisabled(),
        true,
      );
    }
    if (scene === "readonly")
      assert.equal(
        await page
          .getByRole("button", { name: "添加连接", exact: true })
          .isDisabled(),
        true,
      );
    await settings("个人 Passkey");
    if (["error", "offline"].includes(scene)) {
      await page
        .getByText("个人 Passkey 操作未确认或已取消，请重读状态后核对。", {
          exact: true,
        })
        .waitFor();
      assert.equal(
        await page
          .getByRole("button", { name: "通过系统验证解锁", exact: true })
          .count(),
        0,
      );
    }
    if (scene === "readonly")
      await page
        .getByText("当前装配未提供个人 Passkey 能力", { exact: true })
        .waitFor();
    if (scene === "empty") {
      await page
        .getByRole("button", { name: "通过系统验证解锁", exact: true })
        .click();
      await page.getByText("还没有网站 Passkey", { exact: true }).waitFor();
    }
    await settings("运行与诊断");
    if (["error", "offline"].includes(scene)) {
      await page
        .getByText("无法读取 AI Host 状态，请重试。", { exact: true })
        .waitFor();
      await page
        .getByRole("button", { name: "预览脱敏诊断", exact: true })
        .click();
      const diagnostic = page.getByRole("dialog");
      await diagnostic
        .getByText("无法生成诊断预览，请重试。", { exact: true })
        .waitFor();
      assert.equal(
        await diagnostic
          .getByRole("button", { name: "保存此脱敏快照", exact: true })
          .count(),
        0,
      );
      await page.keyboard.press("Escape");
      await diagnostic.waitFor({ state: "hidden" });
    }
    await settings("账号与组织");
    await page
      .getByRole("heading", { name: "账号与组织", exact: true })
      .waitFor();
    await settings("外观与体验");
    await page
      .getByRole("combobox", { name: "界面主题", exact: true })
      .waitFor();
    const submitted = await page.evaluate(() =>
      window.rssPageFixture.events.filter((event) =>
        [
          "software_submit",
          "policy_invoke",
          "network_run",
          "settings_change",
          "query_submit",
          "diagnostics_export",
        ].includes(event.operation),
      ),
    );
    assert.deepEqual(
      submitted,
      [],
      `${scene}: inspection must not submit a device operation or export`,
    );
  }
  evidence.checks.push(
    "independent error/offline/empty/partial/read-only behaviors across software, system, tools, connections, Vault, diagnostics, account and local appearance; no unintended submissions",
  );
  writeFileSync(
    new URL("react-pages-evidence.json", directory),
    JSON.stringify(evidence, null, 2),
  );
  console.log(
    "PASS React business pages: explicit original operations, secrets and diagnostics boundaries, v4 page matrix 1280/720/420 light/dark and empty/partial/error/offline/read-only. OS effects and Windows native remain separate.",
  );
}
