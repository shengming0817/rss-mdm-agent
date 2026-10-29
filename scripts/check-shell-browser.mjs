import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
export async function checkShellBrowser(page, fixture) {
  const root = new URL(fixture.url).origin;
  const go = async (parameters = "") => {
    await page.goto(fixture.url + "?desktop" + parameters);
    await page.getByRole("button", { name: "切换侧栏布局" }).waitFor();
    await page.getByRole("heading", { name: /好/ }).waitFor();
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
  await page
    .getByRole("textbox", { name: "消息" })
    .fill("附上页面展示信息，先不发送");
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
        await page.screenshot({
          path: new URL(
            `../.local-ci-runs/react-shell-${platform}-${theme}-${width}.png`,
            import.meta.url,
          ).pathname,
        });
      }
    }
  }
  await go();
  await page.setViewportSize({ width: 720, height: 900 });
  await cycle.click();
  await cycle.click();
  assert.equal(await shell.getAttribute("data-layout-step"), "2");
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
  await go();
  await page.getByRole("button", { name: "用户菜单", exact: true }).click();
  await page
    .getByRole("menuitem", { name: "设备注册与 MDM", exact: true })
    .click();
  await page.getByRole("button", { name: "注册这台设备", exact: true }).click();
  await page
    .getByRole("heading", { name: "等待组织授权", exact: true })
    .waitFor();
  assert.equal(
    await page
      .getByRole("button", { name: "注册这台设备", exact: true })
      .isDisabled(),
    true,
  );
  assert.equal(
    await page
      .getByRole("heading", { name: "设备已注册", exact: true })
      .count(),
    0,
  );
  console.log(
    "PASS v4 shell/home: fixed logo and cycle coordinates, full primary/secondary access, directed restore, retained scroll/drafts, search keyboard, page assistant, account/client isolation, registration Unknown, macOS/Windows visual previews, 1280/720/420 light/dark. Windows native remains separate.",
  );
}
