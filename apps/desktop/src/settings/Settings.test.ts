import { mount, flushPromises } from "@vue/test-utils";
import { it, expect, vi } from "vitest";
import type { RuntimeClient, SessionView } from "@rss-mdm-agent/ai-client";
import type { Connection, HostStatus } from "@rss-mdm-agent/ai-contract";
import { activeStage } from "@rss-mdm-agent/ai-contract";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
import { createAssistant } from "../assistant/controller";
import { createHostSettings } from "./controller";
import Settings from "./Settings.vue";
async function setup() {
  let rows: Connection[] = [];
  const session = fixtureSession();
  const view: SessionView = {
    namespace: session.namespace,
    selectedConnectionId: "cfg",
    generation: activeStage(session).binding.generation,
    cursor: 0,
    capabilities: activeStage(session).capabilities,
    sessionStatus: "active",
    connection: "attached",
    timeline: [],
    commands: {},
    messages: {},
    interactions: {},
    surfaces: {},
    tools: {},
  };
  const client = {
    initialize: async () => ({}),
    connections: async () => ({
      connections: rows,
      preferences: { schemaVersion: 6, kind: "userPreferences" },
    }),
    listSessions: async () => ({ items: [] }),
    observe: () => () => {},
    close: vi.fn(),
    connection: { closed: new Promise(() => {}) },
    savePreferences: vi.fn(),
    testConnection: vi.fn(async () => {
      rows = [{ ...rows[0], configRevision: 2, status: "ready" }];
      return rows[0];
    }),
    createSession: vi.fn().mockResolvedValue(view),
    saveConnection: vi.fn(
      async (row: import("@rss-mdm-agent/ai-contract").ConnectionDraft) => {
        rows = [
          {
            ...row,
            schemaVersion: 6,
            kind: "connection",
            configRevision: 1,
            status: "unverified",
          },
        ];
        return rows[0];
      },
    ),
  } as unknown as RuntimeClient;
  const c = createAssistant(
    { connect: async () => ({ runtime: client, mode: "s1" }) },
    () => "cfg",
  );
  await c.connect();
  const status: HostStatus = {
    schemaVersion: 6,
    kind: "hostStatus",
    generation: 1,
    phase: "ready",
    source: "bundled_resource",
    version: "test",
    recent: [],
  };
  const host = createHostSettings({
    read: async () => status,
    restart: async () => status,
    export: async () => true,
  });
  await host.refresh();
  const wrapper = mount(Settings, {
    props: { host, assistant: c },
    attachTo: document.body,
  });
  const button = (name: string) =>
    wrapper.findAll("button").find((b) => b.text() === name)!;
  const fill = async () => {
    const name = wrapper
      .findAll("label")
      .find((l) => l.text().startsWith("名称"))!;
    await name.get("input").setValue("Connection");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
  };
  return {
    c,
    client,
    host,
    wrapper,
    button,
    fill,
    close() {
      wrapper.unmount();
      c.dispose();
      host.dispose();
    },
  };
}
it("first use may defer or validate then create exactly one pending conversation through the settings CTA", async () => {
  const t = await setup();
  try {
    expect(t.button("新建对话并前往 AI").attributes("disabled")).toBeDefined();
    await t.button("稍后配置，前往 AI").trigger("click");
    expect(t.wrapper.emitted("assistant")).toHaveLength(1);
    expect(t.client.createSession).not.toHaveBeenCalled();
    await t.fill();
    expect(t.client.saveConnection).toHaveBeenCalledTimes(1);
    expect(t.button("新建对话并前往 AI").attributes("disabled")).toBeDefined();
    await t.button("测试连接").trigger("click");
    await flushPromises();
    expect(
      t.button("新建对话并前往 AI").attributes("disabled"),
    ).toBeUndefined();
    let complete!: (v: SessionView) => void;
    const view = await t.client.createSession();
    vi.mocked(t.client.createSession)
      .mockClear()
      .mockImplementation(() => new Promise((r) => (complete = r)));
    await t.button("新建对话并前往 AI").trigger("click");
    await t.button("新建对话并前往 AI").trigger("click");
    expect(t.client.createSession).toHaveBeenCalledTimes(1);
    complete(view);
    await flushPromises();
    expect(t.wrapper.emitted("assistant")).toHaveLength(2);
  } finally {
    t.close();
  }
});
it("keeps the connection draft editable while the AI Host is disconnected", async () => {
  const t = await setup();
  try {
    t.c.state.connection = "disconnected";
    await flushPromises();

    const form = t.wrapper.get("form.connection-form");
    const name = form.get("input");
    expect(form.element.closest("fieldset")?.hasAttribute("disabled")).toBe(
      false,
    );
    await name.setValue("Offline draft");
    expect((name.element as HTMLInputElement).value).toBe("Offline draft");
    expect(t.button("保存配置").attributes("disabled")).toBeDefined();
  } finally {
    t.close();
  }
});
it("restart and connection deletion dialogs trap both tab directions, escape and restore focus", async () => {
  const t = await setup();
  try {
    await t.fill();
    for (const trigger of [
      t.button("重启 AI Host"),
      t.wrapper.get('button[aria-label="删除连接 Connection"]'),
    ]) {
      (trigger.element as HTMLElement).focus();
      await trigger.trigger("click");
      await flushPromises();
      const dialog = t.wrapper.get('[role="alertdialog"]');
      const buttons = dialog.findAll("button");
      const first = buttons[0],
        last = buttons.at(-1)!;
      expect(document.activeElement).toBe(first.element);
      await first.trigger("keydown", { key: "Tab", shiftKey: true });
      expect(document.activeElement).toBe(last.element);
      await last.trigger("keydown", { key: "Tab" });
      expect(document.activeElement).toBe(first.element);
      await first.trigger("keydown", { key: "Escape" });
      await flushPromises();
      expect(t.wrapper.find('[role="alertdialog"]').exists()).toBe(false);
      expect(document.activeElement).toBe(trigger.element);
    }
  } finally {
    t.close();
  }
});

it("projects connection requirements into visible labels and associated help", async () => {
  const t = await setup();
  try {
    const label = (prefix: string) =>
      t.wrapper.findAll("label").find((l) => l.text().startsWith(prefix))!;
    expect(label("名称").text()).toContain("必填");
    expect(label("模型").text()).toContain("可选");
    await label("认证来源").get("select").setValue("custom_api");
    for (const prefix of ["API 地址", "API Key / Auth Token", "模型"]) {
      const field = label(prefix);
      expect(field.text()).toContain("必填");
      const input = field.get("input");
      const helpId = input.attributes("aria-describedby");
      expect(helpId).toBeTruthy();
      expect(t.wrapper.get(`#${helpId}`).text()).toContain("必填");
    }
    expect(label("模型").get("input").attributes("placeholder")).not.toContain(
      "留空",
    );
    await label("认证来源").get("select").setValue("existing_config");
    expect(label("模型").text()).toContain("可选");
    expect(label("模型").get("input").attributes("required")).toBeUndefined();
  } finally {
    t.close();
  }
});
it("explains connection capacity failures without the history size message", async () => {
  const t = await setup();
  try {
    vi.mocked(t.client.saveConnection).mockRejectedValueOnce({
      code: "limit_exceeded",
    });
    await t.fill();
    expect(t.wrapper.text()).toContain("连接数量已达上限");
    expect(t.wrapper.text()).toContain("删除不用的连接");
    expect(t.wrapper.text()).not.toContain("64 KiB");
  } finally {
    t.close();
  }
});
