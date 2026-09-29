import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi, beforeEach } from "vitest";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
import type { Connection } from "@rss-mdm-agent/ai-contract";
import Connections from "../settings/ConnectionSettings.vue";
import { createAssistant } from "./controller";
import { saveNativeConnection } from "../test-users";
vi.mock("../test-users", () => ({
  nativeTestMode: true,
  saveNativeConnection: vi.fn(),
}));
beforeEach(() => vi.mocked(saveNativeConnection).mockReset());
const existing: Connection = {
  schemaVersion: 7,
  kind: "connection",
  connectionId: "custom",
  configRevision: 1,
  name: "Custom",
  provider: "codex",
  source: {
    type: "custom_api",
    apiUrl: "https://old.example/v1",
    model: "chosen",
    credentialType: "api_key",
  },
  status: "ready",
  profile: "conversation",
};
async function setup(rows: Connection[] = []) {
  const client = {
    initialize: async () => ({}),
    connections: async () => ({
      connections: rows,
      preferences: { schemaVersion: 7, kind: "userPreferences" },
    }),
    listSessions: async () => ({ items: [] }),
    observe: () => () => {},
    close: vi.fn(),
    connection: { closed: new Promise(() => {}) },
    testConnection: vi.fn(),
  } as unknown as RuntimeClient;
  const c = createAssistant(
    { connect: async () => ({ runtime: client, mode: "s1" }) },
    () => "fixture-id",
  );
  await c.connect();
  const w = mount(Connections, { props: { controller: c, active: true } });
  const field = (name: string) =>
    w
      .findAll("label")
      .find((l) => l.text().startsWith(name))!
      .get("input,select");
  const button = (name: string) =>
    w.findAll("button").find((b) => b.text() === name)!;
  const custom = async () => {
    await field("认证来源").setValue("custom_api");
    await field("名称").setValue("Custom");
    await field("API 地址").setValue("https://example.invalid/v1");
    await field("模型").setValue("chosen");
  };
  return {
    c,
    client,
    w,
    field,
    button,
    custom,
    close: () => {
      w.unmount();
      c.dispose();
    },
  };
}
it("reveals the credential entry immediately, preserves pasted bytes and clears a submitted secret on failure", async () => {
  const t = await setup();
  vi.mocked(saveNativeConnection).mockRejectedValueOnce({
    code: "unavailable",
  });
  try {
    await t.custom();
    expect(t.w.findAll('input[type="password"]')).toHaveLength(1);
    expect(t.w.text()).toContain("未设置");
    const password = t.field("API Key");
    await password.trigger("paste");
    await password.setValue("  synthetic-paste  ");
    await t.button("显示凭据").trigger("click");
    expect(password.attributes("type")).toBe("text");
    await t.button("隐藏凭据").trigger("click");
    expect(password.attributes("type")).toBe("password");
    await t.w.get("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(saveNativeConnection).mock.calls.length).toBe(1);
    const [draft, revision, secret] =
      vi.mocked(saveNativeConnection).mock.calls[0];
    expect(secret === "  synthetic-paste  ").toBe(true);
    expect(
      "status" in draft || "configRevision" in draft || "secret" in draft,
    ).toBe(false);
    expect(revision).toBeNull();
    expect((password.element as HTMLInputElement).value).toBe("");
    expect((t.field("名称").element as HTMLInputElement).value).toBe("Custom");
    expect(t.client.testConnection).not.toHaveBeenCalled();
  } finally {
    t.close();
  }
});
it("retains saved credentials but requires new input when the endpoint changes", async () => {
  const t = await setup([existing]);
  vi.mocked(saveNativeConnection).mockResolvedValue(existing);
  try {
    await t.button("编辑").trigger("click");
    expect(t.w.text()).toContain("已安全保存");
    expect((t.field("API Key").element as HTMLInputElement).value).toBe("");
    await t.w.get("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(saveNativeConnection).mock.calls[0][2]).toBeUndefined();
    await t.button("编辑").trigger("click");
    await t.field("API 地址").setValue("https://new.example/v1");
    await t.w.get("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(saveNativeConnection).mock.calls.length).toBe(1);
    await t.field("API Key").setValue("replacement");
    await t.w.get("form").trigger("submit");
    await flushPromises();
    expect(
      vi.mocked(saveNativeConnection).mock.calls[1][2] === "replacement",
    ).toBe(true);
  } finally {
    t.close();
  }
});
it("clears secrets on cancel, navigation, target change and unmount", async () => {
  const t = await setup([existing]);
  try {
    await t.button("编辑").trigger("click");
    await t.button("更换凭据").trigger("click");
    await t.field("API Key").setValue("synthetic");
    await t.w.setProps({ active: false });
    expect((t.field("API Key").element as HTMLInputElement).value).toBe("");
    await t.w.setProps({ active: true });
    await t.field("API Key").setValue("synthetic");
    await t.field("服务").setValue("claude");
    expect((t.field("API Key").element as HTMLInputElement).value).toBe("");
    await t.field("API Key").setValue("synthetic");
    await t.button("清空表单").trigger("click");
    await t.custom();
    expect((t.field("API Key").element as HTMLInputElement).value).toBe("");
    await t.field("API Key").setValue("synthetic");
    const input = t.field("API Key").element as HTMLInputElement;
    t.w.unmount();
    expect(input.value).toBe("");
  } finally {
    t.c.dispose();
  }
});
it("a pending model test does not disable editing or saving", async () => {
  const t = await setup([existing]);
  vi.mocked(saveNativeConnection).mockResolvedValue(existing);
  vi.mocked(t.client.testConnection).mockReturnValue(new Promise(() => {}));
  try {
    await t.button("测试连接").trigger("click");
    await t.button("编辑").trigger("click");
    await t.field("名称").setValue("Edited");
    await t.w.get("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(saveNativeConnection).mock.calls.length).toBe(1);
  } finally {
    t.close();
  }
});
