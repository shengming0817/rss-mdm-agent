// Injected only by custom-connection-acceptance into the real bundled WebView.
(async () => {
  let stage = "load";
  const ipcOutputs = [];
  const originalInvoke = window.__TAURI_INTERNALS__.invoke;
  // Tauri's invoke is immutable. Observe the real callbacks without replacing
  // transport or replies; this hook exists only in the acceptance WebView.
  const callbacks = window.__TAURI_INTERNALS__.callbacks;
  const register = callbacks.set.bind(callbacks);
  let overflow = false;
  callbacks.set = (id, callback) =>
    register(id, (value) => {
      if (ipcOutputs.length < 512) ipcOutputs.push(value);
      else overflow = true;
      return callback(value);
    });
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
    await click("设置");
    const panel = await wait(() =>
      document.querySelector(".settings .connections"),
    );
    for (const details of panel.querySelectorAll("details"))
      details.open = true;
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
    window.__RSS_OBSERVED_OUTPUTS__ = {
      dom: document.documentElement.outerHTML,
      ipc: ipcOutputs,
      status: await originalInvoke("ai_host_status"),
    };
    if (
      overflow ||
      !ipcOutputs.some(
        (value) =>
          value?.ok === true &&
          value.value?.kind === "connection" &&
          value.value.status === "ready" &&
          value.value.name === "Local DeepSeek protocol fixture",
      )
    )
      throw new Error("save IPC output not observed");
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
