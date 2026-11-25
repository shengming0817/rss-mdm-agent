// Runs in the isolated organization webview. The organization owns the session and
// operation cookies. ref: Web Authentication Level 3, §5.1.8 and §5.1.10.
async function employeeBrowser(input) {
  if (window !== window.top || location.origin !== input.origin) return;
  const identity =
    "/api/v1/identity/tenants/" + encodeURIComponent(input.tenant);
  const base =
    "/api/v1/tenants/" +
    encodeURIComponent(input.tenant) +
    "/devices/" +
    encodeURIComponent(input.device) +
    "/device-access";
  const resource = base + "/bindings/" + encodeURIComponent(input.binding);
  document.querySelector("dialog[data-rss-employee]")?.remove();
  const dialog = document.createElement("dialog");
  dialog.dataset.rssEmployee = "true";
  const heading = document.createElement("h2");
  heading.textContent =
    input.change.action.kind === "reset_factors"
      ? "重置本机 RSS 登录因子"
      : "修复本人设备注册";
  const message = document.createElement("p");
  const controls = document.createElement("div");
  const close = document.createElement("button");
  close.textContent = "关闭";
  close.onclick = () => dialog.close();
  dialog.append(heading, message, controls, close);
  document.body.append(dialog);
  dialog.showModal();

  async function request(path, method = "GET", body, csrf) {
    const response = await fetch(path, {
      method,
      credentials: "same-origin",
      redirect: "error",
      cache: "no-store",
      headers: {
        "x-identity-request": "1",
        ...(csrf && { "x-csrf-token": csrf }),
        ...(body && { "content-type": "application/json" }),
      },
      ...(body && { body: JSON.stringify(body) }),
    });
    if (!response.ok) {
      if (response.status === 401)
        throw Error("请登录当前组织或重新完成本次操作的身份验证。");
      if ([403, 404, 409, 422].includes(response.status))
        throw Error("组织未批准这次操作，请刷新本人绑定并检查当前账号。");
      throw Error("未收到确认。请返回桌面查询原操作。");
    }
    return response.json();
  }
  async function session() {
    const s = await request(identity + "/session");
    if (s.identity?.principalId !== input.principal || !s.csrfToken)
      throw Error("请在此组织登录与桌面相同的企业账号。");
    return s.csrfToken;
  }
  function showError(error) {
    message.textContent =
      error.name === "NotAllowedError" || error.name === "AbortError"
        ? "验证已取消或超时。可重新验证本次操作。"
        : error.message?.startsWith("请") ||
            error.message?.startsWith("组织") ||
            error.message?.startsWith("未收到")
          ? error.message
          : "验证未完成。可重新验证或查询原操作。";
  }
  function button(label, run) {
    const b = document.createElement("button");
    b.textContent = label;
    b.onclick = async () => {
      for (const item of controls.children) item.disabled = true;
      try {
        await run();
      } catch (error) {
        showError(error);
      } finally {
        for (const item of controls.children) item.disabled = false;
      }
    };
    controls.append(b);
  }
  // WebAuthn Level 2 binary credential API; wire format follows webauthn-rs-proto auth.rs.
  function bytes(encoded) {
    return Uint8Array.from(
      atob(encoded.replace(/-/g, "+").replace(/_/g, "/")),
      (c) => c.charCodeAt(0),
    );
  }
  function encoded(buffer) {
    let raw = "";
    for (const byte of new Uint8Array(buffer)) raw += String.fromCharCode(byte);
    return btoa(raw).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }
  try {
    await session();
    message.textContent = "选择本人验证方式。验证通过后，返回桌面提交原操作。";
    button("使用已有通行密钥", async () => {
      const token = await session();
      const challenge = await request(
        resource + "/step-up",
        "POST",
        input.change,
        token,
      );
      try {
        const options = challenge.options.publicKey;
        const publicKey = {
          ...options,
          challenge: bytes(options.challenge),
          allowCredentials: options.allowCredentials?.map((c) => ({
            ...c,
            id: bytes(c.id),
          })),
        };
        const c = await navigator.credentials.get({ publicKey });
        if (!c || c.type !== "public-key") throw Error();
        const r = c.response;
        const response = {
          id: c.id,
          type: c.type,
          rawId: encoded(c.rawId),
          response: {
            authenticatorData: encoded(r.authenticatorData),
            clientDataJSON: encoded(r.clientDataJSON),
            signature: encoded(r.signature),
            userHandle: r.userHandle ? encoded(r.userHandle) : null,
          },
          clientExtensionResults: c.getClientExtensionResults(),
        };
        await request(
          identity + "/webauthn/authenticate/finish",
          "POST",
          { id: challenge.id, response },
          token,
        );
      } catch (error) {
        await request(
          identity +
            "/webauthn/" +
            encodeURIComponent(challenge.id) +
            "/cancel",
          "POST",
          {},
          token,
        ).catch(() => {});
        throw error;
      }
      message.textContent = "本人验证已完成。请返回桌面提交这次操作。";
    });
    if (!input.returnTarget) return;
    const options = await request(identity + "/login-options");
    for (const provider of options.providers) {
      button("使用 " + provider.label + " 验证", async () => {
        const grant = await request(
          resource + "/step-up/oidc/" + encodeURIComponent(provider.providerId),
          "POST",
          { change: input.change, returnTarget: input.returnTarget },
          await session(),
        );
        const destination = new URL(grant.authorizationUrl);
        if (
          destination.protocol !== "https:" ||
          destination.username ||
          destination.password
        )
          throw Error();
        location.assign(destination.href);
      });
    }
  } catch (error) {
    showError(error);
  }
}
