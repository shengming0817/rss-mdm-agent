import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
export async function checkShellBrowser(page, fixture) {
  const root = new URL(fixture.url).origin;
  const go = async (parameters = "") => {
    await page.goto(fixture.url + "?desktop" + parameters);
    await page.getByRole("button", { name: "切换侧栏布局" }).waitFor();
    await page.getByRole("heading", { name: /好/ }).waitFor();
    await page.locator(".fixture-controls").evaluate((node) => {
      node.style.display = "none";
    });
  };
  mkdirSync(new URL("../.local-ci-runs/", import.meta.url), {
    recursive: true,
  });
  const capture = async (name) => {
    await page.evaluate(async () => {
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
    });
    await page.screenshot({
      path: new URL(
        `../.local-ci-runs/react-shell-${name}.png`,
        import.meta.url,
      ).pathname,
    });
  };
  const shell = page.locator(".rss-desktop-shell"),
    cycle = page.getByRole("button", { name: "切换侧栏布局" });
  await go();
  await page
    .getByRole("button", { name: "查看 Workspace", exact: true })
    .waitFor();
  const before = await cycle.boundingBox();
  for (const step of [1, 2, 3, 0]) {
    await cycle.click();
    assert.equal(await shell.getAttribute("data-layout-step"), String(step));
    if (step === 2) await capture("full-sidebar");
    const position = await cycle.boundingBox();
    assert.equal(position.x, before.x);
    assert.equal(position.y, before.y);
    assert.equal(
      await page.locator('.rss-brand[aria-label="RSS"]').isVisible(),
      true,
    );
  }
  await page.keyboard.press("Control+b");
  assert.equal(await shell.getAttribute("data-layout-step"), "3");
  await cycle.click();
  assert.equal(await shell.getAttribute("data-layout-step"), "0");
  const primary = page.locator(".rss-hidden-primary");
  assert.deepEqual(
    await primary
      .locator("button.rss-primary-button")
      .evaluateAll((nodes) =>
        nodes.map((node) => node.getAttribute("aria-label")),
      ),
    ["主页", "AI 助手", "软件中心", "工具中心", "系统设置", "任务中心"],
  );
  await primary.getByRole("button", { name: "软件中心", exact: true }).click();
  await page
    .locator(".rss-hidden-secondary")
    .getByRole("button", { name: "更新", exact: true })
    .click();
  await primary.getByRole("button", { name: "主页", exact: true }).click();
  await primary.getByRole("button", { name: "软件中心", exact: true }).click();
  assert.equal(
    await page
      .locator('.rss-hidden-secondary button[aria-current="page"]')
      .textContent(),
    "更新",
  );
  await primary.getByRole("button", { name: "主页", exact: true }).click();
  await page
    .getByRole("textbox", { name: "主页助手草稿" })
    .fill("保留原主体的草稿");
  const home = page.locator('.rss-page-viewport[aria-label="主页"]');
  await page.setViewportSize({ width: 1280, height: 700 });
  await home.evaluate((node) => {
    node.scrollTop = 160;
  });
  const scroll = await home.evaluate((node) => node.scrollTop);
  assert.ok(scroll > 0);
  for (let i = 0; i < 4; i++) await cycle.click();
  assert.equal(await home.evaluate((node) => node.scrollTop), scroll);
  assert.equal(
    await page.getByRole("textbox", { name: "主页助手草稿" }).inputValue(),
    "保留原主体的草稿",
  );
  await page.keyboard.press("Control+k");
  const search = page.getByRole("combobox", { name: "搜索应用、工具和设置" });
  await search.press("ArrowUp");
  await page.waitForFunction(() => {
    const list = document.querySelector(".rss-search-results"),
      option = list?.querySelector('[aria-selected="true"]');
    if (!list || !option) return false;
    const outer = list.getBoundingClientRect(),
      inner = option.getBoundingClientRect();
    return inner.top >= outer.top && inner.bottom <= outer.bottom;
  });
  await capture("search-keyboard");
  await search.fill("Workspace");
  await search.press("Enter");
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  assert.equal(
    await page
      .locator('.rss-hidden-secondary button[aria-current="page"]')
      .textContent(),
    "已安装",
  );
  await primary.getByRole("button", { name: "主页", exact: true }).click();
  await page.getByRole("button", { name: "页面助手", exact: true }).click();
  await page.getByRole("complementary", { name: "页面助手" }).waitFor();
  await page.waitForFunction(
    () => !window.rssDesktop.state.workspace.assistant.compositionError(),
  );
  await page.evaluate(async () => {
    await window.rssDesktop.state.workspace.tasks.loadRecent();
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve)),
    );
  });
  assert.equal(
    await page.evaluate(() =>
      window.rssDesktop.state.workspace.assistant.compositionError(),
    ),
    "",
  );
  await page
    .getByRole("textbox", { name: "消息" })
    .fill("附上页面展示信息，先不发送");
  await capture("page-assistant");
  await page.getByRole("button", { name: "移除页面上下文" }).click();
  await page.getByRole("button", { name: "关闭页面助手" }).click();
  assert.equal(
    await page
      .getByRole("button", { name: "页面助手", exact: true })
      .evaluate((node) => node === document.activeElement),
    true,
  );
  mkdirSync(new URL("../.local-ci-runs/", import.meta.url), {
    recursive: true,
  });
  await go("&scene=task-error");
  await page.locator(".rss-home-activity-row").first().click();
  const taskDialog = page.getByRole("dialog", { name: /原任务/ });
  await taskDialog
    .getByText("原任务不可访问或状态未确认，请重试读取", { exact: true })
    .waitFor();
  const originalTask = await page.evaluate(() =>
    JSON.stringify(window.rssDesktop.state.workspace.tasks.state.requested),
  );
  await capture("task-detail-error");
  await taskDialog
    .getByRole("button", { name: "重新读取原任务", exact: true })
    .click();
  await page.waitForFunction(
    () => !!window.rssDesktop.state.workspace.tasks.state.selected,
  );
  assert.equal(
    await page.evaluate(() =>
      JSON.stringify(
        window.rssDesktop.state.workspace.tasks.state.selected.reference,
      ),
    ),
    originalTask,
  );
  await page.keyboard.press("Escape");
  for (const [title, section] of [
    ["账号与组织", "account"],
    ["外观与体验", "appearance"],
    ["设备注册与 MDM", "registration"],
  ]) {
    await page.keyboard.press("Control+k");
    const searchSettings = page.getByRole("combobox", {
      name: "搜索应用、工具和设置",
    });
    await searchSettings.fill(title);
    await searchSettings.press("Enter");
    await page.getByRole("dialog").waitFor({ state: "hidden" });
    assert.equal(
      await page.evaluate(
        () =>
          window.rssDesktop.state.workspace.navigation.state.selected.settings,
      ),
      section,
    );
  }
  const visualMetrics = [];
  for (const platform of ["macos", "windows"]) {
    await go("&platform=" + platform);
    await page
      .getByRole("button", { name: "查看 Workspace", exact: true })
      .waitFor();
    await page.locator(".fixture-controls").evaluate((node) => {
      node.style.display = "none";
    });
    for (const theme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await page.waitForFunction(
        (mode) =>
          document.querySelector(".rss-ui")?.getAttribute("data-theme") ===
          mode,
        theme,
      );
      for (const width of [1280, 720, 420]) {
        await page.setViewportSize({ width, height: 900 });
        const overflow = await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        );
        assert.equal(
          overflow,
          false,
          `${platform}/${theme}/${width} document overflow`,
        );
        await page.evaluate(async () => {
          await document.fonts.ready;
          await new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          );
        });
        const metrics = await page.evaluate(() => {
          const bounds = (selector) => {
            const r = document.querySelector(selector).getBoundingClientRect();
            return { x: r.x, y: r.y, width: r.width, height: r.height };
          };
          return {
            header: bounds(".rss-topbar"),
            footer: bounds(".rss-shell-footer"),
            main: bounds(".rss-shell-main"),
            heroFont: getComputedStyle(
              document.querySelector(".rss-device-hero h2"),
            ).fontSize,
            headers: document.querySelectorAll(".rss-topbar").length,
          };
        });
        visualMetrics.push({ platform, theme, width, ...metrics });
        assert.equal(metrics.headers, 1);
        assert.equal(metrics.header.height, 54);
        assert.equal(metrics.footer.y, 872);
        assert.equal(metrics.heroFont, width > 1180 ? "26px" : "24px");
        await page.screenshot({
          path: new URL(
            `../.local-ci-runs/react-shell-${platform}-${theme}-${width}.png`,
            import.meta.url,
          ).pathname,
        });
      }
    }
  }
  writeFileSync(
    new URL("../.local-ci-runs/react-shell-metrics.json", import.meta.url),
    JSON.stringify(visualMetrics, null, 2),
  );
  await go();
  await page.setViewportSize({ width: 720, height: 900 });
  await cycle.click();
  await cycle.click();
  assert.equal(await shell.getAttribute("data-layout-step"), "2");
  await capture("narrow-sidebar-overlay");
  await page.keyboard.press("Escape");
  assert.equal(await shell.getAttribute("data-layout-step"), "3");
  assert.equal(
    await cycle.evaluate((node) => node === document.activeElement),
    true,
  );
  await page.setViewportSize({ width: 1280, height: 900 });
  await cycle.click();
  await page.evaluate(async () => {
    const assistant = window.rssDesktop.state.workspace.assistant;
    await assistant.create();
    assistant.draft.set("PRIVATE PRE-LOGIN DRAFT");
  });
  assert.equal(
    await page.getByRole("textbox", { name: "主页助手草稿" }).inputValue(),
    "PRIVATE PRE-LOGIN DRAFT",
  );
  assert.ok(
    (await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.sessions().length,
    )) > 0,
  );
  await page.getByRole("button", { name: "用户菜单", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "登录企业账号", exact: true })
    .click();
  await capture("account-login");
  await page.getByRole("textbox", { name: "企业账号" }).fill("scoped-person");
  await page.getByLabel("企业密码", { exact: true }).fill("fixture-secret");
  await page.evaluate(() => {
    window.previousClient = window.assistantRuntime;
  });
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  await page.getByRole("heading", { name: /scoped-person/ }).waitFor();
  await page.waitForFunction(
    () => window.assistantRuntime !== window.previousClient,
  );
  assert.equal(
    await page.getByRole("textbox", { name: "主页助手草稿" }).inputValue(),
    "",
  );
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.sessions().length,
    ),
    0,
  );
  assert.equal(
    (await page.locator("body").innerText()).includes("fixture-secret"),
    false,
  );
  for (const scene of ["empty", "partial", "error"]) {
    await go("&scene=" + scene);
    if (scene === "empty")
      await page.getByText("没有已读取的应用", { exact: true }).waitFor();
    if (scene === "partial")
      await page
        .getByText("已读取部分清单，覆盖尚未完整", { exact: true })
        .waitFor();
    await capture("home-" + scene);
    if (scene === "error") {
      await page.getByText("设备概览读取失败", { exact: true }).waitFor();
      assert.equal(
        (
          await page
            .locator('.rss-page-viewport[aria-label="主页"]')
            .innerText()
        ).includes("Studio Mac"),
        false,
      );
    }
  }
  await fetch(root + "/__fixture/scenario", {
    method: "POST",
    body: JSON.stringify({ scenario: "unregistered" }),
  });
  await go("&scene=registration-wait");
  await page.getByRole("button", { name: "用户菜单", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "设备注册与 MDM", exact: true })
    .click();
  await page.getByRole("button", { name: "注册这台设备", exact: true }).click();
  await page
    .getByRole("button", { name: "停止等待注册", exact: true })
    .waitFor();
  const registrationBefore = await (
    await fetch(root + "/__fixture/registration")
  ).json();
  assert.equal(registrationBefore.phase, "awaitingAuthorization");
  await page.getByRole("button", { name: "停止等待注册", exact: true }).click();
  await page
    .getByRole("button", { name: "继续核验原注册", exact: true })
    .click();
  await page
    .getByRole("heading", { name: "等待组织授权", exact: true })
    .waitFor();
  const registrationAfter = await (
    await fetch(root + "/__fixture/registration")
  ).json();
  assert.equal(registrationAfter.grantId, registrationBefore.grantId);
  assert.equal(
    registrationAfter.publicKeyFingerprint,
    registrationBefore.publicKeyFingerprint,
  );
  assert.equal(registrationAfter.registered, false);
  assert.equal(
    await page
      .getByRole("heading", { name: "设备已注册", exact: true })
      .count(),
    0,
  );
  await capture("registration-pending");
  await page.getByRole("button", { name: "用户菜单", exact: true }).click();
  await page.getByRole("menuitem", { name: "外观与体验", exact: true }).click();
  const themeChoice = page.getByRole("combobox", { name: "界面主题" });
  await themeChoice.click();
  await themeChoice.press("ArrowDown");
  await themeChoice.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector(".rss-ui")?.getAttribute("data-theme") === "light",
  );
  await capture("appearance");
  console.log(
    "PASS v4 shell/home: fixed logo and cycle coordinates, full primary/secondary access, directed restore, retained scroll/drafts, search keyboard, page assistant, account/client isolation, registration Unknown, macOS/Windows visual previews, 1280/720/420 light/dark. Windows native remains separate.",
  );
}
