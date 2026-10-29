import assert from "node:assert/strict";
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
let browser;
try {
  browser = await chromium.launch({ channel: "chrome", headless: true });
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
  await input.fill("发送测试");
  await input.press("Enter");
  await page.locator(".rss-message-user").getByText("发送测试").waitFor();
  assert.equal(await input.inputValue(), "");
  for (const colorScheme of ["light", "dark"]) {
    await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
    await page.waitForFunction(
      (mode) =>
        document.querySelector(".rss-ui")?.getAttribute("data-theme") === mode,
      colorScheme,
    );
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
      await page.screenshot({
        path: new URL(
          `../.local-ci-runs/react-foundation-${colorScheme}-${width}.png`,
          import.meta.url,
        ).pathname,
        fullPage: true,
      });
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
  assert.equal(
    await trigger.evaluate((node) => node === document.activeElement),
    true,
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  console.log(
    "PASS React foundation: v4 light/dark, 1280/720/420, forms, menu/drawer keyboard and focus, draft, message IME unit seam, unknown read-only recovery, inert Markdown. Business pages and native platform acceptance are separate.",
  );
} finally {
  await browser?.close();
  await fixture.close();
}
