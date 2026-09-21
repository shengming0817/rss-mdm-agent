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
  let generation;
  const invoke = (command, args) =>
    window.__TAURI_INTERNALS__.invoke(command, args);
  const current = async () => (await invoke("test_users")).current;
  const selectUser = async (name) => {
    await click("设置");
    const previous = (await current())?.generation;
    const input = await wait(() =>
      document.querySelector('[aria-label="测试用户名"]'),
    );
    input.value = name;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await click("进入");
    await wait(async () => {
      const value = await current();
      if (
        value?.generation !== previous &&
        value?.user.displayName === name &&
        document.querySelector(".self-service .hero")
      )
        return value;
    });
    generation = (await current()).generation;
  };
  const rejects = async (action) => {
    try {
      await action();
    } catch {
      return;
    }
    throw new Error("old scope remained accessible");
  };
  const saveExistingConnection = async () => {
    await click("设置");
    const panel = await wait(() =>
      document.querySelector(".settings .connections"),
    );
    for (const details of panel.querySelectorAll("details"))
      details.open = true;
    for (const [name, value] of [
      ["名称", "Existing Codex"],
      ["配置目录", window.__RSS_CONNECTION_SOURCE__.directory],
      ["工具", "controlled_tools"],
    ]) {
      const el = [...panel.querySelectorAll("label")]
        .find((el) => el.textContent.startsWith(name))
        ?.querySelector("input,select");
      el.value = value;
      el.dispatchEvent(
        new Event(el.tagName === "SELECT" ? "change" : "input", {
          bubbles: true,
        }),
      );
    }
    await click("验证并保存");
    await wait(() => panel.querySelector("li")?.textContent.includes("可用"));
  };
  const snapshot = () =>
    window.__TAURI_INTERNALS__.invoke("self_service_snapshot", {
      input: { after: null, requestIds: [] },
      generation,
    });
  const details = (requestId) =>
    window.__TAURI_INTERNALS__.invoke("execution_task_details", {
      requestId,
      generation,
    });
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
          source.config.id,
          source.config.revision,
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
    if (window.__RSS_ACCEPTANCE_PHASE__ === 0) {
      const input = await wait(() =>
        document.querySelector('[aria-label="测试用户名"]'),
      );
      input.value = "Native acceptance";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await click("进入");
    }
    await wait(() => document.querySelector(".self-service .hero"));
    generation = (await window.__TAURI_INTERNALS__.invoke("test_users")).current
      .generation;
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
      await saveExistingConnection();
      await click("新建对话并前往 AI");
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
      // A streamed marker is not completion. User switching correctly cancels any
      // remaining model work, so durable-history acceptance must await its terminal.
      setStage("ai_terminal");
      await wait(() =>
        [...document.querySelectorAll(".assistant .command-state")].some((e) =>
          e.textContent.includes("模型本轮结束：completed"),
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
      const alice = await current();
      const oldChannel = await invoke("ai_connect", { generation });
      await click("批准此测试计划一次");
      // The durable runner is now in flight. Switching must not change its frozen origin.
      const started = await details("ai-s1-smoke");
      if (
        started.status.attempts !== 1 ||
        started.status.phase === "testCompleted"
      )
        throw new Error("task did not start before switch");
      setStage("user_b_isolation");
      await selectUser("Native acceptance B");
      const bob = await current();
      if (bob.user.userId === alice.user.userId)
        throw new Error("user identity reused");
      await rejects(() =>
        invoke("self_service_snapshot", {
          input: { after: null, requestIds: [] },
          generation: alice.generation,
        }),
      );
      await rejects(() =>
        invoke("ai_send", {
          connectionId: oldChannel,
          message: {
            jsonrpc: "2.0",
            id: "late",
            method: "initialize",
            params: {},
          },
        }),
      );
      await rejects(() => details("ai-s1-smoke"));
      if ((await snapshot()).requests.length)
        throw new Error("foreign tasks visible");
      await click("AI 助手");
      await wait(() => document.querySelector(".settings .connections"));
      await wait(() => button("新建会话"));
      if (
        document.querySelector(".connections li") ||
        document.querySelector(".assistant-sessions li") ||
        document.querySelector(".assistant .message")
      )
        throw new Error("foreign AI state visible");
      // Both users intentionally choose the same real CLI login, with independent catalogs/history.
      await saveExistingConnection();
      await click("新建对话并前往 AI");
      const draft = await wait(() =>
        document.querySelector(".assistant textarea"),
      );
      draft.value = "B_UNSENT_DRAFT_MUST_NOT_CROSS_USERS";
      draft.dispatchEvent(new Event("input", { bubbles: true }));
      setStage("user_a_restored");
      await selectUser("Native acceptance");
      if (
        (await current()).user.userId !== alice.user.userId ||
        generation === alice.generation
      )
        throw new Error("original user not restored with fresh generation");
      setStage("user_a_task_continued");
      const completed = await wait(async () => {
        const value = await details("ai-s1-smoke");
        return value.status.phase === "testCompleted" && value;
      });
      if (
        JSON.stringify(completed.plan) !== JSON.stringify(ai.plan) ||
        completed.status.attempts !== 1
      )
        throw new Error("original task identity changed");
      await wait(
        async () =>
          (await details("ai-s1-tool")).status.phase === "testCompleted",
      );
      await click("AI 助手");
      setStage("user_a_history");
      await wait(() =>
        [...document.querySelectorAll(".assistant .message")].some((e) =>
          e.textContent.includes("RSS_S1_DONE"),
        ),
      );
      if (document.querySelector(".assistant textarea")?.value)
        throw new Error("draft crossed generation");
      if (
        document.querySelectorAll(".connections li").length !== 1 ||
        document.querySelectorAll(".assistant-sessions li").length !== 1
      )
        throw new Error("catalog or sessions not isolated");
      setStage("same_user_reconnect");
      const beforeReconnect = await details("ai-s1-smoke");
      const selectedBefore = document.querySelector(
        '.assistant-sessions button[aria-current="true"]',
      ).textContent;
      const commandsBefore = document.querySelectorAll(
        ".assistant .command-state",
      ).length;
      await click("设置");
      await click("重新连接");
      await click("AI 助手");
      await wait(() =>
        document
          .querySelector(".assistant-facts")
          ?.textContent.includes("连接：已连接"),
      );
      await wait(() =>
        [...document.querySelectorAll(".assistant .message")].some((e) =>
          e.textContent.includes("RSS_S1_DONE"),
        ),
      );
      if (
        document.querySelector(
          '.assistant-sessions button[aria-current="true"]',
        ).textContent !== selectedBefore ||
        document.querySelectorAll(".assistant .command-state").length !==
          commandsBefore ||
        JSON.stringify(await details("ai-s1-smoke")) !==
          JSON.stringify(beforeReconnect)
      )
        throw new Error("reconnect changed facts");
      setStage("host_restart");
      const oldHost = await invoke("ai_host_status");
      const oldTask = await details("ai-s1-smoke");
      await click("设置");
      await click("重启 AI Host");
      await click("确认重启");
      await wait(async () => {
        const status = await invoke("ai_host_status");
        return (
          status.phase === "ready" && status.generation > oldHost.generation
        );
      });
      await click("AI 助手");
      await wait(() => button("新建会话"));
      if (
        JSON.stringify(await details("ai-s1-smoke")) !== JSON.stringify(oldTask)
      )
        throw new Error("restart changed device task");
      await wait(() =>
        [...document.querySelectorAll(".assistant .message")].some((e) =>
          e.textContent.includes("RSS_S1_DONE"),
        ),
      );
      setStage("new_conversation_after_restart");
      await click("新建会话");
      const restartedInput = await wait(() => {
        const el = document.querySelector(".assistant textarea");
        return visible(el) && el;
      });
      restartedInput.value =
        "Reply with RSS_RESTART_OK only. Do not use tools.";
      restartedInput.dispatchEvent(new Event("input", { bubbles: true }));
      await click("发送");
      await wait(() =>
        [...document.querySelectorAll(".assistant .command-state")].some((e) =>
          e.textContent.includes("模型本轮结束：completed"),
        ),
      );
      await wait(() =>
        [...document.querySelectorAll(".assistant .message")].some((e) =>
          e.textContent.includes("RSS_RESTART_OK"),
        ),
      );
      const restartSession = document
        .querySelector('.assistant-sessions button[aria-current="true"]')
        .childNodes[0].textContent.trim();
      report({
        step: "passed",
        humanCompleted: true,
        aiApprovedOnce: true,
        sharedCatalog: true,
        frozenOriginsVisible: true,
        detachedRunContinued: true,
        nativeHistoryVisible: true,
        modelCompletedBeforeSwitch: true,
        testTasks: 2,
        userIsolation: true,
        oldGenerationRejected: true,
        originalTaskContinued: true,
        originalHistoryRestored: true,
        sameUserReconnected: true,
        hostRestarted: true,
        restartPreservedTask: true,
        newModelSessionAfterRestart: true,
        restartSession,
        alice: alice.user.userId,
        bob: bob.user.userId,
      });
    }
  } catch {
    report({ step: "failed", stage });
  }
})();
