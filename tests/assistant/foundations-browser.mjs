import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";

export async function checkFoundations(browser, url) {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 820 },
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const select = async (id) => {
    await page
      .getByRole("navigation", { name: "组件族", exact: true })
      .getByRole("button")
      .nth(Number(id.slice(1)) - 1)
      .click();
    await page.locator(`[data-family="${id}"]`).waitFor();
  };
  try {
    mkdirSync(".local-ci-runs/ui-foundations", { recursive: true });
    for (const platform of ["windows", "macos"]) {
      await page.goto(`${url}?foundations=${platform}`);
      for (let index = 1; index <= 26; index++) {
        const id = `U${String(index).padStart(2, "0")}`;
        await select(id);
        assert.ok(
          (await page.locator(".foundation-example").innerText()).trim() ||
            (await page.locator(".foundation-example input").count()),
        );
        assert.equal(
          await page.evaluate(
            () => document.documentElement.scrollWidth > innerWidth,
          ),
          false,
          `${platform}/${id} overflows`,
        );
      }
      await select("U04");
      const input = page.getByLabel("单行文字", { exact: true });
      await input.fill("changed");
      await page.getByLabel("错误", { exact: true }).check();
      assert.equal(await input.getAttribute("aria-invalid"), "true");
      assert.equal(
        await input.getAttribute("aria-describedby"),
        await page.getByRole("alert").getAttribute("id"),
      );
      await page.getByLabel("只读（适用输入）", { exact: true }).check();
      assert.equal(await input.getAttribute("readonly"), "");
      await page.getByLabel("只读（适用输入）", { exact: true }).uncheck();
      await page.getByLabel("错误", { exact: true }).uncheck();
      await select("U12");
      await page.getByLabel("必填字段", { exact: true }).fill("required label");
      await select("U07");
      await page.getByText("复选框", { exact: true }).click();
      assert.equal(
        await page
          .getByRole("checkbox", { name: "复选框", exact: true })
          .getAttribute("aria-checked"),
        "true",
      );
      await page.getByRole("button", { name: "混合状态", exact: true }).click();
      assert.equal(
        await page
          .getByRole("checkbox", { name: "复选框", exact: true })
          .getAttribute("aria-checked"),
        "mixed",
      );
      await select("U09");
      await page.getByRole("combobox", { name: "选择器", exact: true }).click();
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("Enter");
      assert.equal(await page.getByRole("listbox").count(), 0);
      await select("U13");
      await page
        .getByRole("option", { name: "选项 A", exact: true })
        .dblclick();
      await page.getByText("激活 a", { exact: true }).waitFor();
      await select("U14");
      const branch = page.getByRole("treeitem", { name: /分组/ });
      await branch.focus();
      await page.keyboard.press("ArrowRight");
      await page.getByRole("treeitem", { name: /由调用方加载/ }).waitFor();
      await select("U16");
      await page.getByRole("tab", { name: "选项 A", exact: true }).focus();
      await page.keyboard.press("ArrowRight");
      assert.equal(
        await page
          .getByRole("tab", { name: "选项 B", exact: true })
          .getAttribute("aria-selected"),
        "true",
      );
      await select("U19");
      await page.getByRole("separator").focus();
      await page.keyboard.press("End");
      assert.equal(
        await page.getByRole("separator").getAttribute("aria-valuenow"),
        "80",
      );
      await select("U20");
      const menu = page.getByRole("button", { name: "打开菜单", exact: true });
      await menu.click();
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("Escape");
      await page.waitForFunction(
        (el) => el === document.activeElement,
        await menu.elementHandle(),
      );
      await select("U22");
      const trigger = page.getByRole("button", {
        name: "打开对话框",
        exact: true,
      });
      await trigger.click();
      const dialog = page.getByRole("dialog", {
        name: "示例对话框",
        exact: true,
      });
      await dialog.waitFor();
      for (let index = 0; index < 8; index++) {
        await page.keyboard.press(index % 2 ? "Shift+Tab" : "Tab");
        assert.equal(
          await dialog.evaluate((el) => el.contains(document.activeElement)),
          true,
        );
      }
      await dialog.getByRole("combobox").click();
      await page.keyboard.press("Escape");
      assert.equal(
        await dialog.isVisible(),
        true,
        "nested Escape keeps the dialog open",
      );
      await page.keyboard.press("Escape");
      await dialog.waitFor({ state: "detached" });
      await page.waitForFunction(
        (el) => el === document.activeElement,
        await trigger.elementHandle(),
      );
      await page.getByLabel("减少动态", { exact: true }).check();
      await page.getByLabel("高对比", { exact: true }).check();
      await select("U23");
      assert.equal(
        await page
          .locator(".ui-spinner")
          .evaluate((el) => getComputedStyle(el).animationName),
        "none",
      );
      await page.screenshot({
        path: `.local-ci-runs/ui-foundations/${platform}.png`,
        fullPage: true,
      });
      await page.setViewportSize({ width: 550, height: 820 });
      await select("U26");
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        ),
        false,
      );
      await page.setViewportSize({ width: 1100, height: 820 });
    }
    await page.goto(`${url}?foundations=both&family=U22`);
    for (const platform of ["windows", "macos"])
      await page
        .frameLocator(`iframe[title="${platform} 基础控件"]`)
        .getByRole("button", { name: "打开对话框", exact: true })
        .click();
    for (const platform of ["windows", "macos"])
      assert.equal(
        await page
          .frameLocator(`iframe[title="${platform} 基础控件"]`)
          .getByRole("dialog", { name: "示例对话框", exact: true })
          .isVisible(),
        true,
      );
    await page.goto(`${url}?foundations=windows&family=U21`);
    await page.getByRole("button", { name: "打开浮层", exact: true }).click();
    await select("U01");
    assert.equal(
      await page.locator('.ui-overlays [role="dialog"]').count(),
      0,
      "unmounted floating content is removed",
    );
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
}
