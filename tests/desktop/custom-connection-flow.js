// Injected only by custom-connection-acceptance into the real bundled WebView.
(async () => {
  let stage = "load";
  const report = (value) => {
    document.title = "RSS_CUSTOM_CONNECTION:" + JSON.stringify(value);
  };
  const progress = (value) => {
    stage = value;
    report({ step: "progress", stage });
  };
  const wait = async (check, timeout = 120000) => {
    const end = Date.now() + timeout;
    while (Date.now() < end) {
      const value = await check();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("timeout");
  };
  const visible = (element) =>
    element && element.getClientRects().length && !element.disabled;
  const button = (text, root = document) =>
    [...root.querySelectorAll("button")].find(
      (element) => visible(element) && element.textContent.trim() === text,
    );
  const click = async (text, root = document) =>
    (await wait(() => button(text, root))).click();
  const set = (panel, label, value) => {
    const control = [...panel.querySelectorAll("label")]
      .find((element) => element.textContent.startsWith(label))
      ?.querySelector("input,select");
    if (!control) throw new Error("missing control");
    control.value = value;
    control.dispatchEvent(
      new Event(control.tagName === "SELECT" ? "change" : "input", {
        bubbles: true,
      }),
    );
  };
  try {
    progress("select_user");
    const user = await wait(() =>
      document.querySelector('[aria-label="测试用户名"]'),
    );
    user.value = "Custom connection acceptance";
    user.dispatchEvent(new Event("input", { bubbles: true }));
    await click("进入");
    await wait(() => document.querySelector(".self-service .hero"));

    progress("open_connections");
    await click("AI 助手");
    const panel = await wait(() =>
      document.querySelector(".connections details"),
    );
    panel.open = true;
    set(panel, "名称", "Local DeepSeek protocol fixture");
    set(panel, "服务", "deepseek");
    await wait(() =>
      [...panel.querySelectorAll("label")].some((element) =>
        element.textContent.startsWith("API 地址"),
      ),
    );
    set(panel, "API 地址", window.__RSS_CUSTOM_CONNECTION__.apiUrl);
    set(panel, "模型", window.__RSS_CUSTOM_CONNECTION__.model);

    progress("secure_entry");
    await click("验证并保存", panel);
    const row = await wait(() =>
      [...panel.querySelectorAll("li")].find(
        (element) =>
          element.textContent.includes("Local DeepSeek protocol fixture") &&
          element.textContent.includes("可用"),
      ),
    );
    progress("delete");
    button("删除", row).click();
    const confirmation = await wait(() =>
      panel.querySelector('[role="alertdialog"][aria-label="删除连接确认"]'),
    );
    await click("确认删除", confirmation);
    await wait(
      () =>
        ![...panel.querySelectorAll("li")].some((element) =>
          element.textContent.includes("Local DeepSeek protocol fixture"),
        ),
    );
    report({
      step: "passed",
      secureEntry: true,
      modelProbe: "local_openai_compatible_protocol",
      deleted: true,
    });
  } catch (error) {
    report({ step: "failed", stage, reason: "acceptance_flow_failed" });
  }
})();
