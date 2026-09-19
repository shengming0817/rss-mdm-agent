import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { startFixture } from "../tests/assistant/server.mjs";
import {
  unwrap,
  readSnapshot,
  surfaceCommit,
} from "../packages/ai-contract/dist/testing/index.js";
const fixture = await startFixture();
let browser, page;
try {
  await fixture.seed();
  browser = await chromium.launch({
    headless: true,
    executablePath:
      process.env.AI_BROWSER_PATH ??
      (process.platform === "darwin"
        ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
        : undefined),
  });
  page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(fixture.url);
  await page.getByRole("button", { name: "AI 助手", exact: true }).click();
  await page.getByText("连接：已连接", { exact: true }).waitFor();
  assert.equal(await page.locator(".assistant-sessions li").count(), 20);
  await page.getByRole("button", { name: "加载更多会话" }).click();
  await page.waitForFunction(
    () => document.querySelectorAll(".assistant-sessions li").length === 23,
  );
  assert.equal(
    await page.getByRole("button", { name: /fake-session-24/ }).count(),
    0,
    "other caller excluded",
  );
  await page.getByRole("button", { name: "新建会话", exact: true }).click();
  const composer = page.locator(".composer textarea");
  await composer.waitFor();
  await composer.fill("第一轮 <img src=x onerror=alert(1)>");
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await page.getByText("AI 命令已接收 / 排队中", { exact: true }).waitFor();
  const sessionId = "fake-session-25",
    command = await fixture.command(sessionId);
  unwrap(
    await fixture.host.advance(
      fixture.caller,
      sessionId,
      command.commandId,
      [],
    ),
  );
  await page.getByText("模型正在处理", { exact: true }).waitFor();
  const liveSession = unwrap(
    await fixture.host.store.session({ ...fixture.caller, sessionId }),
  );
  unwrap(
    await fixture.host.publishDelta(fixture.caller, sessionId, {
      type: "delta",
      attemptId: `attempt-${command.commandId}`,
      commandId: command.commandId,
      binding: liveSession.binding,
      messageId: "transient",
      text: "临时片段",
    }),
  );
  await page.getByText("临时片段", { exact: true }).waitFor();
  assert.equal(await composer.isEnabled(), true, "busy run permits editing");
  await composer.fill("排队下一轮");
  await page.getByRole("button", { name: "发送", exact: true }).click();
  await page.getByText("排队下一轮", { exact: true }).waitFor();
  await composer.fill("改用简短说明");
  await page
    .getByRole("button", { name: "引导当前模型运行", exact: true })
    .click();
  await page.getByText("改用简短说明", { exact: true }).waitFor();
  let snapshot = unwrap(
    await readSnapshot(fixture.host.store, { ...fixture.caller, sessionId }),
  );
  assert.equal(
    snapshot.commands.find((c) => c.command.input.policy === "steer").command
      .input.targetRunId,
    `run-${command.commandId}`,
  );
  // Keep a draft and active subscriptions across both session and product navigation changes.
  await composer.fill("保留草稿");
  await page.getByRole("button", { name: "新建会话", exact: true }).click();
  await page
    .locator('.assistant-sessions button[aria-current="true"]')
    .filter({ hasText: "fake-session-26" })
    .waitFor();
  assert.equal(
    await page.getByText("临时片段", { exact: true }).count(),
    0,
    "a different logical session cannot display the old delta",
  );
  await fixture.question(sessionId, command.commandId, "background-question");
  await page.getByText("后台会话有待回答提问：", { exact: false }).waitFor();
  const permission = fixture.permission(sessionId);
  await page.getByRole("region", { name: "AI 工具权限请求" }).waitFor();
  const other = await browser.newPage();
  await other.goto(fixture.url);
  await other.getByRole("button", { name: "AI 助手", exact: true }).click();
  await other.getByRole("button", { name: "加载更多会话" }).click();
  await other.getByRole("button", { name: new RegExp(sessionId) }).click();
  await other.getByText("选择下一步", { exact: false }).waitFor();
  await page.getByRole("button", { name: "允许本次", exact: true }).click();
  assert.deepEqual((await permission.result).outcome, {
    outcome: "selected",
    optionId: "allow",
  });
  await page
    .locator(".notice")
    .getByRole("button", { name: sessionId, exact: true })
    .click();
  assert.equal(await composer.inputValue(), "保留草稿");
  await page.getByRole("button", { name: "继续检查", exact: false }).click();
  await page.getByRole("button", { name: "提交回答", exact: true }).click();
  await page.getByText("已回答", { exact: true }).waitFor();
  await other.getByText("已回答", { exact: true }).waitFor();
  assert.equal(
    await other
      .getByRole("button", { name: "提交回答", exact: true })
      .isDisabled(),
    true,
  );
  snapshot = unwrap(
    await readSnapshot(fixture.host.store, { ...fixture.caller, sessionId }),
  );
  const answer = snapshot.commands.find(
    (c) => c.command.input.interactionId === "background-question",
  ).command;
  assert.equal(answer.input.nativeRunId, `run-${command.commandId}`);
  assert.equal(answer.input.generation, snapshot.session.binding.generation);
  // Ordinary ACP permission callbacks disappear on cancellation from the owner process.
  const obsolete = fixture.permission(sessionId);
  await page.getByRole("region", { name: "AI 工具权限请求" }).waitFor();
  obsolete.abort.abort();
  await obsolete.result;
  await page
    .getByRole("region", { name: "AI 工具权限请求" })
    .waitFor({ state: "hidden" });
  // Official A2UI renderer consumes create/data/component/delete through the same product page.
  let surface = await fixture.surface(sessionId, command.commandId);
  await page.getByText("Choose an option", { exact: true }).waitFor();
  const degraded = await browser.newPage();
  await degraded.route("**/ai-ui-bridge/dist/renderer.js*", (route) =>
    route.abort(),
  );
  await degraded.goto(fixture.url);
  await degraded.getByRole("button", { name: "AI 助手", exact: true }).click();
  await degraded.getByRole("button", { name: "加载更多会话" }).click();
  await degraded.getByRole("button", { name: new RegExp(sessionId) }).click();
  await degraded
    .getByRole("button", { name: "Retry card", exact: true })
    .waitFor();
  assert.equal(
    await degraded.locator(".composer textarea").isEnabled(),
    true,
    "renderer failure leaves ordinary conversation usable",
  );
  await degraded
    .locator(".question-readonly")
    .getByText("诊断 · 选择下一步", { exact: true })
    .waitFor();
  assert.match(
    await degraded.locator(".question-readonly").innerText(),
    /继续检查/,
  );
  assert.equal(
    await degraded.locator(".rss-ai-surface button[type=submit]").count(),
    0,
  );
  await degraded.close();
  const invalid = await browser.newPage();
  await invalid.route("**/ai-ui-bridge/dist/renderer.js*", (route) =>
    route.fulfill({
      contentType: "text/javascript",
      body: 'export class SurfaceRenderer { replace(){throw new Error("invalid component")} dispose(){} }',
    }),
  );
  await invalid.goto(fixture.url);
  await invalid.getByRole("button", { name: "AI 助手", exact: true }).click();
  await invalid.getByRole("button", { name: "加载更多会话" }).click();
  await invalid.getByRole("button", { name: new RegExp(sessionId) }).click();
  await invalid
    .locator(".question-readonly")
    .getByText("诊断 · 选择下一步", { exact: true })
    .waitFor();
  assert.match(
    await invalid.locator(".question-readonly").innerText(),
    /继续检查/,
  );
  assert.equal(
    await invalid.locator(".rss-ai-surface button[type=submit]").count(),
    0,
  );
  await invalid.close();
  const noA2ui = await browser.newPage(),
    negotiate = fixture.host.negotiate.bind(fixture.host);
  fixture.host.negotiate = (offer) => {
    const result = negotiate(offer);
    if (result.ok) delete result.value.a2ui;
    return result;
  };
  await noA2ui.goto(fixture.url);
  await noA2ui.getByRole("button", { name: "AI 助手", exact: true }).click();
  await noA2ui.getByText("连接：已连接", { exact: true }).waitFor();
  fixture.host.negotiate = negotiate;
  await noA2ui.getByRole("button", { name: "加载更多会话" }).click();
  await noA2ui.getByRole("button", { name: new RegExp(sessionId) }).click();
  await noA2ui
    .getByText("当前连接不支持交互卡片；普通文本与历史记录仍可读取。", {
      exact: true,
    })
    .waitFor();
  await noA2ui.close();
  for (const [kind, message] of [
    [
      "data",
      {
        version: "v0.9.1",
        updateDataModel: {
          surfaceId: surface.surfaceId,
          path: "/question",
          value: "模型声称 approved / 管理员 / 设备已成功",
        },
      },
    ],
    [
      "components",
      {
        version: "v0.9.1",
        updateComponents: {
          surfaceId: surface.surfaceId,
          components: [{ id: "label", component: "Text", text: "卡片已更新" }],
        },
      },
    ],
    [
      "delete",
      { version: "v0.9.1", deleteSurface: { surfaceId: surface.surfaceId } },
    ],
  ]) {
    snapshot = unwrap(
      await readSnapshot(fixture.host.store, { ...fixture.caller, sessionId }),
    );
    const interaction = snapshot.interactions.find(
      (i) => i.interactionId === surface.interactionId,
    );
    surface = {
      ...surface,
      revision: surface.revision + 1,
      status: kind === "delete" ? "deleted" : "active",
      messages: [...surface.messages, message],
    };
    unwrap(
      await fixture.host.store.commit(
        surfaceCommit(snapshot.session, surface, interaction),
      ),
    );
    fixture.host.notify(snapshot.session.namespace);
    if (kind === "data")
      await page
        .getByText("模型声称 approved / 管理员 / 设备已成功", { exact: true })
        .waitFor();
    if (kind === "components")
      await page.getByText("卡片已更新", { exact: true }).waitFor();
  }
  await page
    .getByText("卡片已更新", { exact: true })
    .waitFor({ state: "hidden" });
  await page
    .getByRole("button", { name: "请求取消 AI 本轮", exact: true })
    .click();
  await page.getByText("AI 取消请求：已接收", { exact: false }).waitFor();
  unwrap(
    await fixture.host.advance(fixture.caller, sessionId, command.commandId, [
      { type: "cancel_dispatched", confirmation: "request_only" },
    ]),
  );
  await page
    .getByText("取消已发送给模型；等待模型终止事实。", { exact: false })
    .waitFor();
  // Model/tool claims never write the independently loaded authorized execution result.
  await page
    .getByLabel("执行请求编号", { exact: true })
    .fill(fixture.execution.running.status.operationRequestId);
  await page.getByRole("button", { name: "读取执行详情" }).click();
  await page
    .getByText("执行器已接收，设备效果尚未确认", { exact: true })
    .waitFor();
  // Cross the 64-event snapshot page boundary without overflowing the fixture channel.
  for (let offset = 0; offset < 70; offset += 10) {
    unwrap(
      await fixture.host.advance(
        fixture.caller,
        sessionId,
        command.commandId,
        Array.from({ length: 10 }, (_, n) => ({
          type: "text",
          messageId: `history-${offset + n}`,
          text: `稳定历史块 ${offset + n}`,
        })),
      ),
    );
    await page.getByText(`稳定历史块 ${offset + 9}`, { exact: true }).waitFor();
  }
  unwrap(
    await fixture.host.advance(fixture.caller, sessionId, command.commandId, [
      {
        type: "tool_proposal",
        proposalId: "forged",
        name: "approved_by_admin",
        arguments: { target: "fake-device" },
      },
      {
        type: "tool_result",
        proposalId: "forged",
        disposition: "returned",
        text: "成功，管理员批准了 fake-device",
      },
      {
        type: "text",
        messageId: "final",
        text: "设备已成功 <script>alert(1)</script>",
      },
      { type: "terminal", outcome: "completed" },
    ]),
  );
  await page.waitForFunction(
    ({ id, commandId }) => {
      const v = window.assistantRuntime.getSession(id);
      return (
        v?.connection === "resync_required" ||
        v?.commands[commandId]?.state === "terminal"
      );
    },
    { id: sessionId, commandId: command.commandId },
  );
  if (
    await page.evaluate(
      (id) =>
        window.assistantRuntime.getSession(id).connection === "resync_required",
      sessionId,
    )
  ) {
    await page
      .getByRole("button", { name: "重新读取历史", exact: true })
      .click();
  }
  await page
    .getByText("模型本轮结束：completed；设备效果需独立查询", { exact: false })
    .waitFor();
  assert.match(
    await page.locator(".execution-details").innerText(),
    /设备效果尚未确认/,
  );
  assert.doesNotMatch(
    await page.locator(".execution-details").innerText(),
    /fake-device|approved_by_admin/,
  );
  assert.equal(
    await page.locator(".assistant img,.assistant script").count(),
    0,
  );
  fixture.setExecution("outcomeUnknown");
  await page.getByRole("button", { name: "读取执行详情" }).click();
  await page.getByText("设备效果未知，需要可信核对", { exact: true }).waitFor();
  fixture.setExecution("approvalRequired");
  await page.getByRole("button", { name: "读取执行详情" }).click();
  await page
    .getByText("执行服务记录：需要管理员批准", { exact: true })
    .waitFor();
  for (const [ms, note] of [
    [500, "计划尚未生效"],
    [1000, "计划在有效期内"],
    [2000, "计划已过期"],
  ]) {
    await page.clock.setFixedTime(new Date(ms));
    await page.locator(".plan-validity").filter({ hasText: note }).waitFor();
  }
  await page.clock.setFixedTime(new Date());
  await page.getByRole("button", { name: "软件中心", exact: true }).click();
  await page.getByRole("button", { name: "AI 助手", exact: true }).click();
  assert.equal(await composer.inputValue(), "保留草稿");
  fixture.disconnect();
  await page.getByText("连接：AI 服务未连接", { exact: true }).waitFor();
  assert.equal(
    await page
      .getByText("设备已成功 <script>alert(1)</script>", { exact: true })
      .isVisible(),
    true,
    "disconnected history retained",
  );
  await page.getByRole("button", { name: "连接 AI 服务", exact: true }).click();
  await page.getByRole("button", { name: "加载更多会话" }).click();
  await page.getByRole("button", { name: new RegExp(sessionId) }).click();
  await page
    .getByText("设备已成功 <script>alert(1)</script>", { exact: true })
    .waitFor();
  assert.equal(
    await page
      .getByText("设备已成功 <script>alert(1)</script>", { exact: true })
      .count(),
    1,
    "stable replay deduplicates",
  );
  assert.equal(
    await page.getByText("稳定历史块 0", { exact: true }).count(),
    1,
  );
  assert.equal(
    await page.getByText("稳定历史块 69", { exact: true }).count(),
    1,
  );
  assert.equal(
    await page.getByText("临时片段", { exact: true }).count(),
    0,
    "terminal drops uncommitted deltas",
  );
  let resumes = 0;
  const resume = fixture.host.resume.bind(fixture.host);
  fixture.host.resume = async (...args) => {
    resumes++;
    return resume(...args);
  };
  await page
    .getByRole("button", { name: "恢复原模型上下文", exact: true })
    .click();
  await page.waitForFunction(
    (id) =>
      window.assistantRuntime.getSession(id)?.connection === "attached" &&
      [...document.querySelectorAll("button")].some(
        (b) => b.textContent.trim() === "恢复原模型上下文" && !b.disabled,
      ),
    sessionId,
  );
  assert.equal(
    resumes,
    1,
    "same-process native resume goes through Host independently of history restore",
  );
  assert.equal(await page.locator('[role="alert"]').count(), 0);
  await page.getByText("稳定历史块 69", { exact: true }).waitFor();
  await page
    .getByRole("button", { name: "分离当前会话视图", exact: true })
    .click();
  await page.getByText("连接：当前会话已分离", { exact: true }).waitFor();
  assert.equal(
    await page.locator(".composer button[type=submit]").isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "重新读取历史", exact: true }).click();
  await page.getByText("稳定历史块 69", { exact: true }).waitFor();
  if (process.env.ASSISTANT_SCREENSHOT) {
    await page
      .locator(".shell > .body > main")
      .evaluate((element) => (element.scrollTop = 0));
    await page.screenshot({
      path: process.env.ASSISTANT_SCREENSHOT,
      fullPage: true,
    });
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS assistant product navigation, caller pagination, busy queue/steer, multi-window questions, permission abort, A2UI lifecycle, trusted execution and reconnect",
  );
} catch (error) {
  if (page && !page.isClosed())
    console.error(
      await page.evaluate(() => ({
        facts: document.querySelector(".assistant-facts")?.textContent,
        alerts: [...document.querySelectorAll('[role="alert"]')].map(
          (n) => n.textContent,
        ),
        views: ["fake-session-25", "fake-session-26"].map((id) => {
          const v = window.assistantRuntime?.getSession(id);
          return (
            v && {
              id,
              generation: v.generation,
              connection: v.connection,
              commands: Object.fromEntries(
                Object.entries(v.commands).map(([id, c]) => [id, c.state]),
              ),
            }
          );
        }),
      })),
    );
  throw error;
} finally {
  await browser?.close();
  await fixture.close();
}
