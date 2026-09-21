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
      preferences: { schemaVersion: 5, kind: "userPreferences" },
    }),
    listSessions: async () => ({ items: [] }),
    observe: () => () => {},
    close: vi.fn(),
    connection: { closed: new Promise(() => {}) },
    savePreferences: vi.fn(),
    createSession: vi.fn().mockResolvedValue(view),
    saveConnection: vi.fn(async (row: Connection) => {
      rows = [{ ...row, status: "ready" }];
      return rows[0];
    }),
  } as unknown as RuntimeClient;
  const c = createAssistant(
    { connect: async () => ({ runtime: client, mode: "s1" }) },
    () => "cfg",
  );
  await c.connect();
  const status: HostStatus = {
    schemaVersion: 5,
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
    expect(t.button("验证并保存").attributes("disabled")).toBeDefined();
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
