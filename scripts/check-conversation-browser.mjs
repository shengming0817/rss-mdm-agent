import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
/** Real RuntimeClient/access-service + scripted Host; no external model or device effect. */
export async function checkConversationBrowser(page, fixture) {
  const directory = new URL("../.local-ci-runs/", import.meta.url);
  mkdirSync(directory, { recursive: true });
  const evidence = {
    assembly:
      "production React assistant + real RuntimeClient/access service + scripted Memory Host; operation ports are explicit samples",
    providerProof: false,
    osEffectProof: false,
    captures: [],
    checks: [],
  };
  const primary = (name) =>
    page
      .locator(".rss-hidden-primary")
      .getByRole("button", { name, exact: true })
      .click();
  const secondary = (name) =>
    page
      .locator(".rss-hidden-secondary")
      .getByRole("button", { name, exact: true })
      .click();
  const input = page
    .locator(".rss-assistant-input")
    .getByRole("textbox", { name: "消息", exact: true });
  const drive = async (sessionId, input) => {
    const response = await fetch(new URL("/__fixture/assistant", fixture.url), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId, ...input }),
    });
    assert.equal(response.ok, true, `scripted Host stimulus ${input.action}`);
    return response.json();
  };
  const send = async (text) => {
    await secondary("新会话");
    await input.fill(text);
    await input.press("Enter");
    await page.waitForFunction(() => {
      const c = window.rssDesktop.state.workspace.assistant;
      return (
        c.state.selected &&
        Object.values(c.view()?.commands ?? {}).some(
          (v) => v.command.input.type === "prompt",
        )
      );
    });
    return page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.state.selected,
    );
  };
  const capture = async (name, state) => {
    await page.evaluate(async () => {
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      );
    });
    const file = `react-assistant-${name}-${state}.png`;
    const geometry = await page.evaluate(() => {
      const bounds = (selector) => {
        const r = document.querySelector(selector).getBoundingClientRect();
        return { x: r.x, y: r.y, width: r.width, height: r.height };
      };
      const history = document.querySelector(".rss-session-history");
      return {
        viewport: innerWidth,
        documentWidth: document.documentElement.scrollWidth,
        conversation: bounds(".rss-assistant-conversation"),
        composer: bounds(".rss-assistant-input"),
        history: history.hidden ? null : bounds(".rss-session-history"),
        topMenus: document.querySelectorAll(
          ".rss-assistant-conversation .rss-page-header",
        ).length,
      };
    });
    assert.ok(
      geometry.documentWidth <= geometry.viewport + 1,
      `${name}/${state}: document overflow`,
    );
    assert.ok(geometry.composer.width > 200);
    assert.equal(geometry.topMenus, 0);
    if (geometry.history) {
      assert.equal(
        geometry.history[geometry.viewport <= 720 ? "height" : "width"],
        geometry.viewport <= 720 ? 225 : geometry.viewport <= 960 ? 240 : 260,
      );
      assert.ok(geometry.conversation.height > 250);
    }
    await page.screenshot({
      path: new URL(file, directory).pathname,
      animations: "disabled",
      style: ".fixture-controls { display:none!important; }",
    });
    evidence.captures.push({ file, ...geometry });
  };
  for (const platform of ["macos", "windows"]) {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto(`${fixture.url}?desktop&platform=${platform}`);
    await page.getByRole("button", { name: "切换侧栏布局" }).waitFor();
    await page.waitForFunction(
      () =>
        window.rssDesktop?.state.workspace?.assistant.state.connection ===
        "connected",
    );
    await page.locator(".fixture-controls").evaluate((node) => {
      node.style.display = "none";
    });
    await primary("AI 助手");
    assert.equal(
      await page.getByRole("complementary", { name: "历史会话清单" }).count(),
      0,
    );
    const sessionId = await send("请说明当前设备状态，并给我下一步建议。");
    const text =
      "设备状态已经读取。\n\n## 建议的下一步\n\n- 先核对原任务和设备观测。\n- 具体操作需要本人确认，模型文本不会授予权限。\n\n```text\n只读状态 · 效果待核实\n```";
    await drive(sessionId, {
      action: "advance",
      events: [
        { type: "text", messageId: "answer", text },
        { type: "terminal", outcome: "completed" },
      ],
    });
    await page
      .getByRole("heading", { name: "建议的下一步", exact: true })
      .waitFor();
    await input.fill("留在当前会话的草稿");
    await secondary("历史会话");
    const history = page.getByRole("complementary", { name: "历史会话清单" });
    await history.waitFor();
    await page
      .locator(".rss-assistant-conversation")
      .click({ position: { x: 10, y: 10 } });
    assert.equal(await history.isVisible(), true);
    assert.equal(
      await history.getByRole("button", { name: /固定|取消固定/ }).count(),
      0,
    );
    await history
      .getByRole("textbox", { name: "搜索历史会话" })
      .fill("当前设备状态");
    assert.ok(
      (await history.locator(".rss-session-history-item").count()) >= 1,
    );
    await history
      .locator('.rss-session-history-item[aria-current="page"]')
      .click();
    assert.equal(await history.isVisible(), true);
    assert.equal(await input.inputValue(), "留在当前会话的草稿");
    await history.getByRole("textbox", { name: "搜索历史会话" }).fill("");
    for (const theme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await page.waitForFunction(
        (theme) =>
          document.querySelector(".rss-ui").getAttribute("data-theme") ===
          theme,
        theme,
      );
      for (const width of [1280, 720, 420]) {
        await page.setViewportSize({ width, height: 900 });
        await capture(`${platform}-${theme}-${width}`, "history");
        await history.getByRole("button", { name: "关闭历史会话" }).click();
        assert.equal(await input.inputValue(), "留在当前会话的草稿");
        await capture(`${platform}-${theme}-${width}`, "chat");
        await secondary("历史会话");
      }
    }
    await page.setViewportSize({ width: 1280, height: 900 });
    await history.getByRole("button", { name: "关闭历史会话" }).click();
    const operationId = crypto.randomUUID();
    await page.evaluate(
      ({ sessionId, operationId }) =>
        window.rssPageFixture.stageAssistantOperation(sessionId, operationId),
      { sessionId, operationId },
    );
    // A fresh active prompt drives the delivery; the authority remains the original record/port.
    await input.fill("请准备组织修复参数");
    await input.press("Enter");
    await page.waitForFunction(() =>
      window.rssDesktop.state.workspace.assistant.busy(),
    );
    await drive(sessionId, {
      action: "delivery",
      operationId,
      proposal: { name: "execution_operation_prepare", arguments: {} },
    });
    const card = page.getByRole("region", { name: "具体操作核对" });
    await card.getByRole("textbox", { name: "修复名称" }).waitFor();
    assert.equal(
      await card.getByRole("textbox", { name: "固定路径" }).isDisabled(),
      true,
    );
    await card
      .getByRole("textbox", { name: "修复名称" })
      .fill("本人核对的最终参数");
    assert.equal(
      await page.evaluate(
        () =>
          window.rssPageFixture.events.filter(
            (e) => e.operation === "ai_operation_submit",
          ).length,
      ),
      0,
    );
    for (const theme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      for (const width of [1280, 720, 420]) {
        await page.setViewportSize({ width, height: 900 });
        await card.evaluate((node) => node.scrollIntoView({ block: "start" }));
        await capture(`${platform}-${theme}-${width}`, "operation");
        await card
          .locator(".rss-ai-operation-footer")
          .evaluate((node) => node.scrollIntoView({ block: "end" }));
        await capture(`${platform}-${theme}-${width}`, "operation-end");
      }
    }
    await card
      .getByRole("button", { name: "确认本次具体动作", exact: true })
      .click();
    await page.waitForFunction(
      () =>
        window.rssPageFixture.events.filter(
          (e) => e.operation === "ai_operation_submit",
        ).length === 1,
    );
    const saved = await page.evaluate(
      (id) => window.rssPageFixture.records.get(id),
      operationId,
    );
    assert.equal(saved.action.choice.parameters.name, "本人核对的最终参数");
    assert.equal(saved.action.choice.parameters.path, "/Applications");
    assert.equal(saved.state, "submitting");
    assert.equal(await page.getByRole("dialog").count(), 0);
    assert.equal(
      await card.getByText("已核实符合预期", { exact: true }).count(),
      0,
    );
    await drive(sessionId, {
      action: "advance",
      events: [{ type: "terminal", outcome: "completed" }],
    });
    await card.getByRole("button", { name: "查看原任务" }).click();
    await page.getByRole("dialog", { name: /任务详情/ }).waitFor();
    await page.waitForFunction(
      (id) =>
        window.rssDesktop.state.workspace.tasks.state.selected?.reference
          .request === id,
      operationId,
    );
    await page.keyboard.press("Escape");
    evidence.checks.push(
      `${platform}: fixed history, retained current draft, 24 captures, trusted delivery to edited original Policy draft and one concrete confirmation; no extra dialog or terminal effect claimed`,
    );
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await primary("AI 助手");
  const plain = await send("普通问答验收");
  await drive(plain, { action: "question" });
  const question = page.getByRole("region", { name: "本次结构化问题" });
  await question.getByRole("button", { name: "继续检查", exact: true }).click();
  await question
    .getByRole("textbox", { name: "选择下一步", exact: true })
    .fill("用户最终答案");
  await question
    .getByRole("button", { name: "提交本次回答", exact: true })
    .click();
  await question.getByText("原问题状态：已回答", { exact: true }).waitFor();
  const permission = fixture.permission(plain);
  await page
    .getByRole("region", { name: "本次工具请求" })
    .getByRole("button", { name: "仅允许本次请求", exact: true })
    .click();
  assert.deepEqual(await permission.result, {
    outcome: { outcome: "selected", optionId: "allow" },
  });
  await drive(plain, {
    action: "advance",
    events: [{ type: "terminal", outcome: "completed" }],
  });
  const surfaceSession = await send("A2UI 输入验收");
  await drive(surfaceSession, { action: "surface" });
  const surface = page.locator(".rss-ai-surface");
  await surface
    .getByRole("textbox", { name: "Answer", exact: true })
    .fill("实际官方 binding 输入");
  await primary("主页");
  await primary("AI 助手");
  assert.equal(
    await surface.getByRole("textbox", { name: "Answer" }).inputValue(),
    "实际官方 binding 输入",
  );
  await surface
    .getByRole("button", { name: "Send answer", exact: true })
    .click();
  await surface.getByText("此问题已经回答。", { exact: true }).waitFor();
  await drive(surfaceSession, {
    action: "advance",
    events: [{ type: "terminal", outcome: "completed" }],
  });
  const stream = await send("流式输出与停止验收");
  const long = Array.from(
    { length: 50 },
    (_, i) => `第 ${i + 1} 段 · 流式阅读中的设备状态。\n\n`,
  ).join("");
  await drive(stream, { action: "delta", messageId: "stream", text: long });
  const viewport = page.getByLabel("会话内容", { exact: true });
  await viewport
    .getByText("第 50 段 · 流式阅读中的设备状态。", { exact: false })
    .waitFor();
  await viewport.evaluate((node) => {
    node.scrollTop = 80;
    node.dispatchEvent(new Event("scroll"));
  });
  const top = await viewport.evaluate((node) => node.scrollTop);
  await drive(stream, {
    action: "delta",
    messageId: "stream",
    text: "新增输出仍应保留阅读位置。",
  });
  await page
    .getByRole("button", { name: "有新内容 · 回到最新", exact: true })
    .waitFor();
  assert.equal(await viewport.evaluate((node) => node.scrollTop), top);
  await page
    .getByRole("button", { name: "有新内容 · 回到最新", exact: true })
    .click();
  assert.ok(await viewport.evaluate((node) => node.scrollTop > 80));
  await page
    .locator(".rss-assistant-input")
    .getByRole("button", { name: "请求取消", exact: true })
    .click();
  await page.waitForFunction(() =>
    Object.values(
      window.rssDesktop.state.workspace.assistant.view().commands,
    ).some((v) => v.command.input.type === "cancel"),
  );
  await drive(stream, {
    action: "advance",
    events: [
      { type: "text", messageId: "stream", text: long + "已记录的最终文本。" },
      { type: "terminal", outcome: "cancelled" },
    ],
  });
  await page.waitForFunction(
    () => !window.rssDesktop.state.workspace.assistant.canCancel(),
  );
  // The same real viewport is reused: React may clamp its old scrollTop before layout cleanup.
  await viewport.evaluate((node) => {
    node.scrollTop = 900;
    node.dispatchEvent(new Event("scroll"));
  });
  const readingPosition = await viewport.evaluate((node) => node.scrollTop);
  assert.equal(readingPosition, 900);
  const short = await send("短会话阅读位置验收");
  await drive(short, {
    action: "advance",
    events: [
      { type: "text", messageId: "short", text: "短回答。" },
      { type: "terminal", outcome: "completed" },
    ],
  });
  await page.waitForFunction(
    () => !window.rssDesktop.state.workspace.assistant.busy(),
  );
  assert.equal(await viewport.evaluate((node) => node.scrollTop), 0);
  await secondary("历史会话");
  const historyReturn = page.getByRole("complementary", {
    name: "历史会话清单",
  });
  await historyReturn
    .getByRole("textbox", { name: "搜索历史会话" })
    .fill(stream);
  await historyReturn.locator(".rss-session-history-item").click();
  await page.waitForFunction(
    (id) =>
      window.rssDesktop.state.workspace.assistant.state.selected === id &&
      !window.rssDesktop.state.workspace.assistant.state.opening,
    stream,
  );
  await historyReturn.getByRole("button", { name: "关闭历史会话" }).click();
  assert.equal(
    await viewport.evaluate((node) => node.scrollTop),
    readingPosition,
    "return from a shorter conversation preserves the prior reading position",
  );
  await capture("history-return-reading", "regression");
  evidence.checks.push(
    "real long-to-short history return: original scrollTop900 restored after shorter content clamps the reused viewport to0; bottom-follow behavior retained in streaming journey",
  );
  evidence.checks.push(
    "real client/Host ordinary answer, one-request permission, official A2UI input/action, stable answer through navigation, genuine transient deltas preserve reading position, explicit latest, original cancellation command and terminal fact",
  );
  // Additional controls use the production settings entry and the same authenticated client/Host.
  const catalog = await fixture.host.store.connections(fixture.caller);
  assert.equal(catalog.ok, true);
  const alternate = "cfg-options-B";
  const seeded = await fixture.host.store.saveConnection(
    fixture.caller,
    {
      ...catalog.value.find((row) => row.connectionId === "cfg"),
      connectionId: alternate,
      presentation: { name: "选项连接 B" },
      configRevision: 1,
    },
    null,
  );
  assert.equal(seeded.ok, true);
  const settings = async () => {
    await page.getByRole("button", { name: "用户菜单", exact: true }).click();
    await page
      .getByRole("menuitem", { name: "AI 与连接", exact: true })
      .click();
  };
  const openOptions = () =>
    page.getByRole("button", { name: "会话选项", exact: true }).click();
  const finishOptions = () =>
    page
      .getByRole("dialog", { name: "会话选项" })
      .getByRole("button", { name: "完成", exact: true })
      .click();
  const finishPrompt = async (sessionId, text, messageId) => {
    await drive(sessionId, {
      action: "advance",
      events: [
        { type: "text", messageId, text },
        { type: "terminal", outcome: "completed" },
      ],
    });
    await page.waitForFunction(
      () => !window.rssDesktop.state.workspace.assistant.busy(),
    );
  };
  const sendCurrent = async (sessionId, text) => {
    const before = (await fixture.command(sessionId)).commandId;
    await input.fill(text);
    await input.press("Enter");
    await page.waitForFunction((previous) => {
      const c = window.rssDesktop.state.workspace.assistant;
      return (
        c.busy() &&
        Object.values(c.view().commands).some(
          (row) =>
            row.command.input.type === "prompt" &&
            row.command.commandId !== previous &&
            row.state !== "terminal",
        )
      );
    }, before);
    const command = await fixture.command(sessionId);
    assert.notEqual(command.commandId, before);
    return command;
  };
  const originalCommand = await fixture.command(stream);
  await settings();
  await page.getByRole("button", { name: "重读连接", exact: true }).click();
  await page
    .getByRole("button", { name: "将 选项连接 B 设为新会话默认", exact: true })
    .click();
  await page.waitForFunction(
    (id) =>
      window.rssDesktop.state.workspace.assistant.state.preferences
        .defaultConnectionId === id,
    alternate,
  );
  const preferences = await fixture.host.store.preferences(fixture.caller);
  assert.equal(preferences.ok, true);
  assert.equal(preferences.value.defaultConnectionId, alternate);
  assert.equal(
    await page.evaluate(() =>
      window.rssDesktop.state.workspace.assistant.selectedConnectionId(),
    ),
    "cfg",
  );
  await openOptions();
  await page.getByRole("combobox", { name: "本会话连接", exact: true }).click();
  await page
    .locator(
      ".ant-select-dropdown:not(.ant-select-dropdown-hidden) .ant-select-item-option-content",
    )
    .getByText("选项连接 B · codex · 版本 1", { exact: true })
    .click();
  await page.getByRole("button", { name: "应用会话连接", exact: true }).click();
  await page.waitForFunction((id) => {
    const c = window.rssDesktop.state.workspace.assistant;
    return (
      c.selectedConnectionId() === id &&
      c.view().connectionPending &&
      !c.state.optionBusy
    );
  }, alternate);
  assert.equal(
    (await fixture.command(stream)).commandId,
    originalCommand.commandId,
  );
  await finishOptions();
  await primary("AI 助手");
  const switched = await sendCurrent(stream, "连接切换后明确发送");
  await finishPrompt(stream, "连接 B 的稳定完成文本。", "options-switched");
  assert.equal(switched.input.type, "prompt");
  await settings();
  await openOptions();
  await page
    .getByRole("button", { name: "下次输入开始新上下文", exact: true })
    .click();
  await page.waitForFunction(() => {
    const c = window.rssDesktop.state.workspace.assistant;
    return c.view().connectionPending && !c.state.optionBusy;
  });
  assert.equal((await fixture.command(stream)).commandId, switched.commandId);
  await page
    .getByRole("spinbutton", { name: "历史轮数", exact: true })
    .fill("1");
  await page
    .getByRole("button", { name: "预览已完成文本", exact: true })
    .click();
  const previewNode = page.locator(".rss-history-preview");
  await previewNode.waitFor();
  assert.ok(
    (await previewNode.innerText()).includes("连接 B 的稳定完成文本。"),
  );
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.state.history.size,
    ),
    0,
  );
  assert.equal((await fixture.command(stream)).commandId, switched.commandId);
  const frozen = await page.evaluate(() =>
    JSON.parse(
      JSON.stringify(
        window.rssDesktop.state.workspace.assistant.state.historyPreview,
      ),
    ),
  );
  assert.equal(frozen.connectionId, alternate);
  assert.deepEqual(frozen.commandIds, [switched.commandId]);
  assert.ok(frozen.text.includes("连接 B 的稳定完成文本。"));
  for (const width of [1280, 420]) {
    await page.setViewportSize({ width, height: 900 });
    const geometry = await page
      .getByRole("dialog", { name: "会话选项" })
      .evaluate((node) => ({
        viewport: innerWidth,
        documentWidth: document.documentElement.scrollWidth,
        drawerWidth: node.getBoundingClientRect().width,
        contentWidth: node.scrollWidth,
      }));
    assert.ok(geometry.documentWidth <= width + 1);
    assert.ok(geometry.drawerWidth <= width + 1);
    assert.ok(geometry.contentWidth <= Math.ceil(geometry.drawerWidth) + 1);
    const file = `react-assistant-session-options-${width}.png`;
    await page.screenshot({
      path: new URL(file, directory).pathname,
      animations: "disabled",
    });
    evidence.captures.push({ file, ...geometry });
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await page
    .getByRole("button", { name: "确认带入这些文本", exact: true })
    .click();
  await finishOptions();
  await primary("AI 助手");
  await page.getByText("下一条输入将带入已确认历史", { exact: true }).waitFor();
  const carried = await sendCurrent(stream, "只带入本人确认的这一次历史");
  assert.deepEqual(carried.input.history, frozen);
  await finishPrompt(stream, "已收到明确带入的文本。", "options-carried");
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.state.history.size,
    ),
    0,
  );
  const plainNext = await sendCurrent(stream, "下一条恢复默认不带历史");
  assert.equal(plainNext.input.history, undefined);
  await finishPrompt(stream, "下一条没有显式历史。", "options-next");
  await settings();
  await openOptions();
  const resumedBefore = (await fixture.command(stream)).commandId;
  await page
    .getByRole("button", { name: "读取原会话状态", exact: true })
    .click();
  await page.waitForFunction(
    () => !window.rssDesktop.state.workspace.assistant.state.opening,
  );
  await page
    .getByRole("button", { name: "恢复原模型上下文", exact: true })
    .click();
  await page.waitForFunction(
    () => !window.rssDesktop.state.workspace.assistant.state.opening,
  );
  assert.equal((await fixture.command(stream)).commandId, resumedBefore);
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.state.selected,
    ),
    stream,
  );
  await finishOptions();
  await primary("AI 助手");
  const failureSession = await send("异步失败诊断");
  const failedCommand = await fixture.command(failureSession);
  await drive(failureSession, {
    action: "advance",
    events: [
      {
        type: "error",
        failure: { code: "unavailable", retry: "reconcile_first" },
      },
      { type: "terminal", outcome: "failed" },
    ],
  });
  await page.getByText(/代码：unavailable/).waitFor();
  await page.getByText(/先读取原会话与原命令/).waitFor();
  assert.equal(
    await page.evaluate(
      () => window.rssDesktop.state.workspace.assistant.state.errors.size,
    ),
    0,
  );
  assert.ok(
    (await page.locator(".rss-ai-run-notes").innerText()).includes(
      failedCommand.commandId,
    ),
  );
  await capture("asynchronous-failure", "diagnostic");
  await settings();
  await page
    .getByRole("button", { name: "配置 选项连接 B", exact: true })
    .click();
  await page.getByRole("button", { name: "移除连接", exact: true }).click();
  await page.locator(".ant-popconfirm-buttons .ant-btn-primary").click();
  await page
    .getByRole("dialog", { name: "配置 AI 连接" })
    .waitFor({ state: "hidden" });
  await page.waitForFunction(
    () =>
      !window.rssDesktop.state.workspace.assistant.state.preferences
        .defaultConnectionId,
  );
  await primary("AI 助手");
  await secondary("新会话");
  assert.equal(
    await page.evaluate(() =>
      window.rssDesktop.state.workspace.assistant.canSend(),
    ),
    false,
  );
  await settings();
  await page
    .getByRole("button", { name: "将 默认连接 设为新会话默认", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      window.rssDesktop.state.workspace.assistant.state.preferences
        .defaultConnectionId === "cfg",
  );
  await primary("AI 助手");
  await input.fill("删除默认连接后选择剩余连接继续");
  await input.press("Enter");
  await page.waitForFunction(() =>
    window.rssDesktop.state.workspace.assistant.busy(),
  );
  const recoveredSession = await page.evaluate(
    () => window.rssDesktop.state.workspace.assistant.state.selected,
  );
  assert.equal(
    await page.evaluate(() =>
      window.rssDesktop.state.workspace.assistant.selectedConnectionId(),
    ),
    "cfg",
  );
  await finishPrompt(
    recoveredSession,
    "使用剩余已验证连接成功发送。",
    "options-default-recovered",
  );
  evidence.checks.push(
    "production user settings + real protocol: default/current choices independent, switching/fresh context never sends, stable history preview requires confirmation and is carried exactly once, original-session readonly/resume never replay prompts, asynchronous Host failure retains code/retry/original ID, deleting default permits explicit remaining-ready selection; 1280/420 drawer captures without horizontal overflow",
  );
  writeFileSync(
    new URL("react-assistant-evidence.json", directory),
    JSON.stringify(evidence, null, 2),
  );
  console.log(
    "PASS React assistant: fixed history, scoped draft, ordinary/A2UI answers, one-shot permissions, trusted R05 action, streaming/stop, session options, exact one-shot history and safe asynchronous diagnostics; v4 matrix and narrow settings captures. Scripted Host and sample records are not provider or OS effect proof.",
  );
}
