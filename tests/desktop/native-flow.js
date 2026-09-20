// Injected only by the native acceptance example into the real bundled WebView.
(async () => {
  let stage = "load";
  const setStage = (value) => {
    stage = value;
    document.title =
      "RSS_ACCEPTANCE:" + JSON.stringify({ step: "progress", stage });
  };
  const wait = async (check, timeout = 120000) => {
    const end = Date.now() + timeout;
    while (Date.now() < end) {
      const value = await check();
      if (value) return value;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("timeout");
  };
  const visible = (el) => el && el.getClientRects().length && !el.disabled;
  const button = (text) =>
    [...document.querySelectorAll("button")].find(
      (b) => visible(b) && b.textContent.trim() === text,
    );
  const click = async (text) => (await wait(() => button(text))).click();
  const snapshot = () =>
    window.__TAURI_INTERNALS__.invoke("self_service_snapshot", {
      input: { after: null, requestIds: [] },
    });
  const details = (requestId) =>
    window.__TAURI_INTERNALS__.invoke("execution_task_details", { requestId });
  const verifyOrigin = async (plan) => {
    await wait(() => {
      const text =
        document.querySelector(".task-detail .request-origin")?.textContent ??
        "";
      const source = plan.initiator;
      const expected = [
        plan.actor,
        plan.authority.id,
        source.kind === "ai" ? "AI 发起" : "人工发起",
        source.osSession.account.subject,
        source.osSession.session,
      ];
      if (source.kind === "ai")
        expected.push(
          source.provider,
          source.providerAccount.account,
          source.providerAccount.config.id,
          source.providerAccount.config.revision,
          source.conversation,
          source.toolCall,
        );
      return expected.every((value) => text.includes(value));
    });
  };
  const report = (value) => {
    document.title = "RSS_ACCEPTANCE:" + JSON.stringify(value);
  };
  try {
    await wait(() => document.querySelector(".self-service .hero"));
    if (window.__RSS_ACCEPTANCE_PHASE__ === 0) {
      setStage("human_preview");
      await click("软件中心");
      const office = await wait(() =>
        [...document.querySelectorAll(".catalog-card")].find(
          (c) => c.querySelector("h2")?.textContent === "办公套件",
        ),
      );
      office.querySelector("button").click();
      await click("预览确定性计划");
      await click("提交测试申请");
      const human = await wait(async () =>
        (await snapshot()).requests.find((r) => r.plan.itemId === "office"),
      );
      if ((await details(human.plan.requestId)).plan.initiator.kind !== "human")
        throw new Error("origin");
      await click("刷新任务");
      (
        await wait(() =>
          [...document.querySelectorAll(".task-row")].find((b) =>
            b.textContent.includes(human.plan.requestId),
          ),
        )
      ).click();
      await verifyOrigin(human.plan);
      await click("批准此测试计划一次");
      await wait(
        async () =>
          (await details(human.plan.requestId)).status.phase ===
          "testCompleted",
      );
      setStage("ai_connect");
      await click("AI 助手");
      await click("新建会话");
      const input = await wait(() => {
        const e = document.querySelector(".assistant textarea");
        return visible(e) && e;
      });
      setStage("ai_submit");
      input.value =
        "使用受控工具完成这次 S1 测试：先读取 execution_catalog，选办公套件 office/test，参数 edition=standard，使用唯一 operationRequestId=ai-s1-smoke；先 execution_preview 冻结，再 execution_submit 提交，保留相同请求和精确 plan。不要批准，不调用本机命令；等待测试管理员的事实是正常结果。另用 operationRequestId=ai-s1-tool 预览并提交 diagnostics/test，参数 host=example.invalid。最后只回复 RSS_S1_DONE，并列出两个原任务编号。";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await click("发送");
      await wait(() =>
        document
          .querySelector(".assistant-facts")
          ?.textContent.includes("本轮处理中"),
      );
      report({ step: "detach", humanCompleted: true });
    } else {
      setStage("ai_reattach");
      await click("AI 助手");
      (
        await wait(() =>
          document.querySelector(".assistant-sessions li button"),
        )
      ).click();
      await wait(() =>
        [...document.querySelectorAll(".assistant .message")].some((e) =>
          e.textContent.includes("RSS_S1_DONE"),
        ),
      );
      setStage("ai_facts");
      const ai = await details("ai-s1-smoke"),
        tool = await details("ai-s1-tool");
      if (
        ai.plan.initiator.kind !== "ai" ||
        !ai.status.submitted ||
        ai.status.attempts !== 0 ||
        ai.status.admission !== "approvalRequired"
      )
        throw new Error("approval");
      if (tool.plan.initiator.kind !== "ai" || !tool.status.submitted)
        throw new Error("tool");
      setStage("shared_task_approval");
      await click("请求与任务");
      await click("刷新任务");
      (
        await wait(() =>
          [...document.querySelectorAll(".task-row")].find((b) =>
            b.textContent.includes("ai-s1-smoke"),
          ),
        )
      ).click();
      await verifyOrigin(ai.plan);
      await click("批准此测试计划一次");
      await wait(
        async () =>
          (await details("ai-s1-smoke")).status.phase === "testCompleted",
      );
      await wait(
        async () =>
          (await details("ai-s1-tool")).status.phase === "testCompleted",
      );
      report({
        step: "passed",
        humanCompleted: true,
        aiApprovedOnce: true,
        sharedCatalog: true,
        frozenOriginsVisible: true,
        detachedRunContinued: true,
        nativeHistoryVisible: true,
        testTasks: 2,
      });
    }
  } catch {
    report({ step: "failed", stage });
  }
})();
