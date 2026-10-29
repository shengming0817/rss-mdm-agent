import assert from "node:assert/strict";
import { checkShellBrowser } from "./check-shell-browser.mjs";
import { mkdirSync } from "node:fs";
import { chromium } from "playwright-core";
import { prepareRun } from "./preparation.mjs";
// Browser evidence covers only the React components currently delivered on the EPIC base.
// Product pages extend this same acceptance entry as they are implemented.
await using run = await prepareRun([
  "node:@rss-mdm-agent/desktop",
  "node:@rss-mdm-agent/ai-access",
]);
const { startFixture } = await import("../tests/assistant/server.mjs");
const fixture = await startFixture();
await fixture.seed();
let browser;
try {
  browser = await chromium.launch({
    headless: true,
    executablePath:
      process.env.AI_BROWSER_PATH ??
      (process.platform === "darwin"
        ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        : undefined),
  });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 900 },
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const remote = [];
  page.on("request", (request) => {
    if (
      !request.url().startsWith(new URL(fixture.url).origin) &&
      !request.url().startsWith("data:")
    )
      remote.push(request.url());
  });
  await page.goto(fixture.url);
  await page.getByRole("heading", { name: "组件基础验收" }).waitFor();
  await page.getByRole("button", { name: "校验表单" }).click();
  await page.getByText("请填写名称").waitFor();
  await page.getByRole("textbox", { name: "名称" }).fill("验证");
  await page.getByRole("button", { name: "校验表单" }).click();
  await page.getByText("表单已校验").waitFor();
  await page.getByRole("button", { name: "查询原请求" }).click();
  await page.getByText("查询次数：1").waitFor();
  assert.equal(
    await page.getByText("执行结束，待核实", { exact: true }).count(),
    1,
  );
  assert.equal(await page.getByText("已请求取消", { exact: true }).count(), 1);
  assert.equal(
    await page
      .locator(".rss-markdown script,.rss-markdown img,.rss-markdown a")
      .count(),
    0,
  );
  const input = page.getByRole("textbox", { name: "消息" });
  await input.fill("第一行");
  await input.press("Shift+Enter");
  assert.equal(await input.inputValue(), "第一行\n");
  await input.fill("第一行\n第二行");
  await input.press("Enter");
  await page.locator(".rss-message-user").getByText("第一行\n第二行").waitFor();
  assert.equal(await input.inputValue(), "");
  assert.equal(
    await page
      .locator(".rss-message-user > p")
      .evaluate((node) => getComputedStyle(node).whiteSpace),
    "pre-wrap",
  );
  for (const colorScheme of ["light", "dark"]) {
    await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
    await page.waitForFunction(
      (mode) =>
        document.querySelector(".rss-ui")?.getAttribute("data-theme") === mode,
      colorScheme,
    );
    const warningBackground =
      colorScheme === "light" ? "rgb(255, 245, 223)" : "rgb(62, 52, 35)";
    const background = await page.waitForFunction((expected) => {
      const alert = document.querySelector(".ant-alert-warning");
      return (
        alert?.isConnected &&
        getComputedStyle(alert).backgroundColor === expected
      );
    }, warningBackground);
    assert.equal(await background.jsonValue(), true);
    for (const width of [1280, 720, 420]) {
      await page.setViewportSize({ width, height: 900 });
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      );
      assert.equal(
        overflow,
        false,
        `${colorScheme}/${width}: horizontal overflow`,
      );
      mkdirSync(new URL("../.local-ci-runs/", import.meta.url), {
        recursive: true,
      });
      // Capture actual scrolled viewports; Chromium's full-page capture can repeat tiles.
      const height = await page.evaluate(
        () => document.documentElement.scrollHeight,
      );
      for (let offset = 0; offset < height; offset += 900) {
        await page.evaluate((y) => window.scrollTo(0, y), offset);
        await page.screenshot({
          path: new URL(
            `../.local-ci-runs/react-foundation-${colorScheme}-${width}${offset ? `-scroll${offset}` : ""}.png`,
            import.meta.url,
          ).pathname,
          fullPage: false,
        });
      }
    }
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  const trigger = page.getByRole("button", { name: "更多操作" });
  await trigger.click();
  await page.getByRole("menuitem", { name: "查看详情" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.waitFor();
  await page.getByRole("textbox", { name: "详情字段" }).fill("不会因主题重置");
  await page.emulateMedia({ colorScheme: "light" });
  assert.equal(
    await page.getByRole("textbox", { name: "详情字段" }).inputValue(),
    "不会因主题重置",
  );
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
  await page.waitForFunction(
    () => document.activeElement?.textContent === "更多操作",
  );
  assert.equal(
    await trigger.evaluate((node) => node === document.activeElement),
    true,
  );
  await page.goto(fixture.url + "?state");
  await page
    .getByTestId("state-connection")
    .getByText("connected", { exact: true })
    .waitFor();
  await page
    .getByRole("textbox", { name: "状态层草稿" })
    .fill("保留原会话草稿");
  await page.getByRole("button", { name: "选择软件更新" }).click();
  assert.equal(
    await page.getByTestId("state-page").textContent(),
    "software:updates",
  );
  for (const expected of ["1:rail", "2:full", "3:rail", "0:hidden"]) {
    await page.getByRole("button", { name: "切换布局" }).click();
    assert.equal(
      await page.getByTestId("state-layout").textContent(),
      expected,
    );
  }
  await page.getByRole("button", { name: "隐藏或恢复侧栏" }).click();
  assert.equal(await page.getByTestId("state-layout").textContent(), "3:rail");
  await page.getByRole("button", { name: "切换布局" }).click();
  assert.equal(
    await page.getByTestId("state-layout").textContent(),
    "0:hidden",
  );
  await page.getByRole("button", { name: "读取软件" }).click();
  await page
    .getByTestId("state-inventory")
    .getByText("4", { exact: true })
    .waitFor();
  await page.getByRole("button", { name: "回到助手" }).click();
  assert.equal(
    await page.getByRole("textbox", { name: "状态层草稿" }).inputValue(),
    "保留原会话草稿",
  );
  assert.ok(Number(await page.getByTestId("state-sessions").textContent()) > 0);
  await checkShellBrowser(page, fixture);
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  console.log(
    "PASS React foundation: v4 light/dark, 1280/720/420, forms, menu/drawer keyboard and focus, draft, message IME unit seam, unknown read-only recovery, inert Markdown. React state owner also connects the real client to the test Host, retains drafts through the directed sidebar cycle and projects async inventory. Business pages and native platform acceptance are separate.",
  );
} finally {
  await browser?.close();
  await fixture.close();
}
