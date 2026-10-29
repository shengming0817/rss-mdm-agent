// Only the explicit fixture assembly imports this adapter. No real IM/ZT discovery or authentication.

import type { UserContext } from "../../packages/ai-contract/src/wire";
import type {
  FixtureLoginPort,
  EnterpriseLogin,
} from "../../apps/desktop/src/account/types";
export function createLoginFixture(
  identity: () => Promise<UserContext>,
): FixtureLoginPort {
  let epoch = 0,
    timer: ReturnType<typeof setTimeout> | undefined,
    organization = "qinghe",
    profile = "normal";
  const state: FixtureLoginPort["state"] = {
    qr: "waiting",
    clients: [],
    discovering: false,
    error: "",
  };
  const organizations = [
    {
      id: "qinghe",
      label: "青禾科技",
      origin: "https://qinghe.example",
      tenantId: "11111111-2222-4333-8444-555555555555",
    },
    {
      id: "beichen",
      label: "北辰设计",
      origin: "https://beichen.example",
      tenantId: "22222222-2222-4333-8444-555555555555",
    },
  ];
  const delay = () => new Promise<void>((resolve) => setTimeout(resolve, 250));
  function cancel() {
    epoch++;
    if (timer) clearTimeout(timer);
    timer = undefined;
    state.current = undefined;
    state.discovering = false;
    state.error = "";
  }
  async function complete(expected: number) {
    await delay();
    if (expected !== epoch) throw new Error("fixture_login_cancelled");
    const current = await identity();
    if (expected !== epoch) throw new Error("fixture_login_cancelled");
    state.current = current;
    return current;
  }
  const port: FixtureLoginPort = {
    state,
    organizations,
    async password(input: EnterpriseLogin) {
      if (!input.login.trim() || !input.password.trim())
        throw new Error("invalid_credentials");
      const expected = epoch;
      return await complete(expected);
    },
    refreshQr() {
      cancel();
      state.qr = "waiting";
      const expected = epoch;
      timer = setTimeout(() => {
        if (expected === epoch) state.qr = "expired";
      }, 120000);
    },
    scan() {
      if (state.qr === "waiting") state.qr = "scanned";
    },
    expire() {
      epoch++;
      state.qr = "expired";
      if (timer) clearTimeout(timer);
    },
    reject() {
      epoch++;
      state.qr = "denied";
      if (timer) clearTimeout(timer);
    },
    async confirm() {
      if (state.qr !== "scanned") return;
      const expected = epoch;
      try {
        await complete(expected);
      } catch {
        state.error = "扫码确认已取消或过期。";
      }
    },
    async discover() {
      cancel();
      state.discovering = true;
      state.clients = [];
      const expected = epoch;
      await delay();
      if (expected !== epoch) return;
      state.discovering = false;
      if (profile === "missing") return;
      if (profile === "error") {
        state.error = "暂时无法读取应用登录状态，请重试。";
        return;
      }
      state.clients = [
        {
          id: "lark",
          name: "飞书",
          kind: "IM",
          account: `林越 · ${organizations.find((o) => o.id === organization)?.label}`,
          status: "signed_in",
        },
        {
          id: "zt",
          name: "企业零信任客户端",
          kind: "零信任",
          account: "林越",
          status: "signed_in",
        },
        {
          id: "wecom",
          name: "企业微信",
          kind: "IM",
          account: "",
          status: "signed_out",
        },
      ];
      if (profile === "signed_out")
        state.clients.forEach((c) => (c.status = "signed_out"));
      if (profile === "mismatch")
        state.clients.forEach((c) => (c.status = "mismatch"));
    },
    scenario(value) {
      profile = value;
      void port.discover();
    },
    async loginClient(id) {
      const client = state.clients.find((c) => c.id === id);
      if (client?.status !== "signed_in" || state.discovering) return;
      const expected = epoch;
      state.discovering = true;
      if (profile === "expired") {
        await delay();
        if (expected !== epoch) return;
        client.status = "expired";
        state.discovering = false;
        state.error = "应用登录状态已失效，请先在该应用中重新登录。";
        return;
      }
      try {
        await complete(expected);
      } finally {
        if (expected === epoch) state.discovering = false;
      }
    },
    cancel,
    organization(id) {
      cancel();
      organization = id;
    },
  };
  return port;
}
