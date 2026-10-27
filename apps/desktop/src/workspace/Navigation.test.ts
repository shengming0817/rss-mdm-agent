import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
import type { PageNavigation } from "./page-navigation";
const navigation = (component: Pick<VueWrapper, "emitted">) =>
  component.emitted("navigation")![0][0] as PageNavigation;
import Workspace from "../Workspace.vue";
import Assistant from "../assistant/Assistant.vue";
import ConversationList from "../assistant/ConversationList.vue";
import SoftwareManagement from "../software/SoftwareManagement.vue";
import ToolsPage from "../device-tools/ToolsPage.vue";
import SystemSettings from "../system-settings/SystemSettings.vue";
import AccountMenu from "./AccountMenu.vue";
import Settings from "../settings/Settings.vue";
import WorkspaceSearch from "./WorkspaceSearch.vue";
import { createHostSettings } from "../settings/controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
async function setup(page = "assistant", fixture = false) {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    new DOMRect(0, 0, 1100, 760),
  );
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page,
      fixture,
      localPorts: fixtureLocalPorts(),
      host: createHostSettings(undefined),
    },
  });
  await flushPromises();
  return w;
}
it("opens history in a separate panel from either sidebar and preserves one composer, sessions and drafts", async () => {
  const w = await setup();
  try {
    const view = w.getComponent(Assistant),
      c = view.props("controller");
    const input = view.get("textarea").element;
    const session = fixtureSession();
    for (const [id, title, time] of [
      ["older", "较早的会话", 1],
      ["latest", "最新的会话", 2],
    ] as const)
      c.state.sessions.set(id, {
        namespace: { ...session.namespace, sessionId: id },
        presentation: { title },
        status: "active",
        lastActivityAtMs: time,
      });
    await c.select("older");
    c.draft.value = "已选会话的草稿";
    await flushPromises();
    expect(w.findComponent(ConversationList).exists()).toBe(false);
    expect(w.get(".workspace-chrome").attributes("data-layout")).toBe("rail");
    await w.get('[data-action="new-conversation"]').trigger("click");
    expect(c.state.selected).toBe("");
    expect(c.state.sessions.size).toBe(2);
    await w.get('[data-action="history-conversations"]').trigger("click");
    expect(w.findAllComponents(ConversationList)).toHaveLength(1);
    expect(w.find(".workspace-rail .conversation-list").exists()).toBe(false);
    expect(w.get(".conversation-history-panel").text()).toContain("较早的会话");
    expect(c.state.selected).toBe("");
    expect(w.get(".conversation-history-panel").classes()).not.toContain(
      "history-pinned",
    );
    await w.get('[aria-label="固定历史会话"]').trigger("click");
    expect(w.get(".conversation-history-panel").classes()).toContain(
      "history-pinned",
    );
    await w.get('[aria-label="取消固定历史会话"]').trigger("click");
    await w.get(".sidebar-cycle").trigger("click");
    expect(w.find(".navigation-panel .conversation-list").exists()).toBe(false);
    expect(
      w.findAll(".navigation-panel .conversation-shortcuts button"),
    ).toHaveLength(2);
    await w.get(".sidebar-cycle").trigger("click");
    expect(w.get(".workspace-chrome").attributes("data-layout")).toBe("hidden");
    const historyChoice = w
      .getComponent(ConversationList)
      .findAll("li button")
      .find((e) => e.text().includes("较早的会话"))!;
    (historyChoice.element as HTMLElement).focus();
    await historyChoice.trigger("click");
    await flushPromises();
    expect(c.state.selected).toBe("older");
    expect(c.draft.value).toBe("已选会话的草稿");
    expect(document.activeElement).toBe(input);
    await w.get(".sidebar-cycle").trigger("click");
    expect(w.get(".workspace-chrome").attributes("data-layout")).toBe("rail");
    expect(w.getComponent(Assistant).get("textarea").element).toBe(input);
    expect(w.findAllComponents(Assistant)).toHaveLength(1);
    w.get(".workspace-chrome").element.dispatchEvent(
      new KeyboardEvent("keydown", { key: "b", ctrlKey: true }),
    );
    await flushPromises();
    expect(w.get(".workspace-chrome").attributes("data-layout")).toBe("hidden");
    w.get(".workspace-chrome").element.dispatchEvent(
      new KeyboardEvent("keydown", { key: "b", ctrlKey: true }),
    );
    await flushPromises();
    expect(w.get(".workspace-chrome").attributes("data-layout")).toBe("rail");
  } finally {
    w.unmount();
  }
});
it("restores each module subsection and category, while ancestor breadcrumbs explicitly return to their target", async () => {
  const w = await setup("software-management", true);
  const root = (label: string) =>
    w
      .findAll(".workspace-rail .ui-navigation button")
      .find((e) => e.text() === label)!;
  try {
    const software = navigation(w.getComponent(SoftwareManagement));
    software.openSection("installed");
    await flushPromises();
    await root("软件管理").trigger("click");
    expect(software.section).toBe("installed");
    software.openSection("enterprise");
    await root("软件管理").trigger("click");
    expect(navigation(w.getComponent(SoftwareManagement)).section).toBe(
      "enterprise",
    );
    const tools = navigation(w.getComponent(ToolsPage));
    tools.openSection("系统维护");
    await root("工具中心").trigger("click");
    expect(tools.section).toBe("系统维护");
    const system = navigation(w.getComponent(SystemSettings));
    await system.openSection("lock");
    await w.setProps({ page: "system-settings" });
    const crumbs = w.get(".workspace-breadcrumb");
    expect(crumbs.text()).toContain("屏幕与睡眠");
    await crumbs.findAll("button")[1].trigger("click");
    expect(system.path.at(-1)?.id).toBe("power");
    await root("系统设置").trigger("click");
    expect(system.section).toBe("power");
    await w.get(".workspace-breadcrumb button").trigger("click");
    expect(system.section).toBe("overview");
    const primary = w.findAll(
      ".workspace-rail .ui-navigation > ol > li > button",
    );
    expect(primary.map((e) => e.text())).toEqual([
      "主页",
      "AI 助手",
      "软件管理",
      "工具中心",
      "任务中心",
      "系统设置",
    ]);
    expect(
      w
        .findAll(".workspace-rail .ui-navigation > ol > li")
        .find((item) => item.find("button").text() === "系统设置")!
        .find(".module-navigation")
        .exists(),
    ).toBe(true);
    await root("工具中心").trigger("click");
    expect(w.emitted("navigate")?.at(-1)).toEqual(["device-tools"]);
    await w.getComponent(AccountMenu).get("button").trigger("click");
    await w
      .get(".workspace-rail .agent-settings-navigation")
      .findAll("button")
      .find((b) => b.text() === "账号与组织")!
      .trigger("click");
    await flushPromises();
    expect(navigation(w.getComponent(Settings)).section).toBe("account");
    expect(w.emitted("navigate")?.at(-1)).toEqual(["settings"]);
  } finally {
    w.unmount();
  }
});
it("opens account settings directly with account first and no popup", async () => {
  const w = await setup("home");
  try {
    await w.getComponent(AccountMenu).get("button").trigger("click");
    expect(w.find(".workspace-rail .agent-settings-navigation").exists()).toBe(
      true,
    );
    expect(w.emitted("navigate")?.at(-1)).toEqual(["settings"]);
    const settings = navigation(w.getComponent(Settings));
    expect(settings.section).toBe("account");
    expect(settings.sections.map((item) => item.id)).toEqual([
      "account",
      "registration",
      "connection",
      "personal-vault",
      "diagnostics",
      "about",
    ]);
    expect(w.find('[role="menu"]').exists()).toBe(false);
    settings.openSection("diagnostics");
    await w.getComponent(AccountMenu).get("button").trigger("click");
    expect(settings.section).toBe("account");
  } finally {
    w.unmount();
  }
});

it("keeps home free of secondary navigation and groups Agent settings above the bottom account entry", async () => {
  const w = await setup("home", true);
  try {
    expect(w.findAll(".workspace-rail .module-navigation")).toHaveLength(0);
    await w.getComponent(AccountMenu).get("button").trigger("click");
    await w.setProps({ page: "settings" });
    const footer = w.get(".workspace-rail .workspace-settings-footer");
    expect(footer.get(".agent-settings-navigation").text()).toContain(
      "开发预览",
    );
    expect(
      w
        .find(".workspace-rail .rail-scroll .agent-settings-navigation")
        .exists(),
    ).toBe(false);
    await footer
      .findAll(".agent-settings-navigation button")
      .find((b) => b.text() === "开发预览")!
      .trigger("click");
    expect(navigation(w.getComponent(Settings)).section).toBe("development");
    expect(w.get(".development-settings").isVisible()).toBe(true);
    await w.setProps({ page: "home" });
    expect(w.get(".development-settings").isVisible()).toBe(false);
    expect(w.get(".shell-status").text()).not.toContain("S1");
  } finally {
    w.unmount();
  }
});
it("opens the same conversation on the right, follows page resources and keeps drafts when closing", async () => {
  const w = await setup("software-management", true);
  try {
    const c = w.getComponent(Assistant).props("controller");
    const input = w.getComponent(Assistant).get("textarea").element;
    const prompt = vi.spyOn(c, "prompt");
    c.draft.value = "尚未发送的问题";
    await w.get('[aria-label="打开电脑助手"]').trigger("click");
    await flushPromises();
    expect(w.find(".page-assistant-panel").exists()).toBe(true);
    expect(w.getComponent(Assistant).get("textarea").element).toBe(input);
    expect(w.findAllComponents(Assistant)).toHaveLength(1);
    expect(c.context.value?.scope).toBe("page");
    expect(c.context.value?.text).toContain("软件管理");
    const nav = navigation(w.getComponent(SoftwareManagement));
    await nav.openResource!(
      "notes",
      nav.searchEntries!.find((item) => item.id === "notes")!.revision,
    );
    await w.getComponent(Assistant).props("controller").removeContext();
    await w.get('[aria-label="附加当前页面上下文"]').trigger("click");
    await flushPromises();
    expect(c.context.value?.text).toContain("Notes");
    await w.setProps({ page: "system-settings" });
    await navigation(w.getComponent(SystemSettings)).openSection("network");
    await flushPromises();
    expect(c.context.value?.stale).toBe(false);
    await w.get('[aria-label="附加当前页面上下文"]').trigger("click");
    expect(c.context.value?.text).toContain("系统设置");
    expect(c.context.value?.text).toContain("网络");
    expect(c.draft.value).toBe("尚未发送的问题");
    expect(prompt).not.toHaveBeenCalled();
    await w.get('[aria-label="关闭电脑助手侧栏"]').trigger("click");
    await flushPromises();
    expect(w.find(".page-assistant-panel").exists()).toBe(false);
    expect(w.getComponent(Assistant).get("textarea").element).toBe(input);
    expect(c.draft.value).toBe("尚未发送的问题");
    expect(w.find(".shell-header .connection-trigger").exists()).toBe(false);
  } finally {
    w.unmount();
  }
});
it("shows AI secondary actions only for the AI module when navigation is hidden", async () => {
  const w = await setup("home");
  try {
    await w.get(".sidebar-cycle").trigger("click");
    await w.get(".sidebar-cycle").trigger("click");
    expect(w.findAll(".chrome-actions .conversation-shortcuts")).toHaveLength(
      0,
    );
    expect(w.findAll(".hidden-secondary .conversation-shortcuts")).toHaveLength(
      0,
    );
    await w.setProps({ page: "assistant" });
    expect(
      w.findAll(".hidden-secondary .conversation-shortcuts button"),
    ).toHaveLength(2);
    await w.setProps({ page: "software-management" });
    expect(w.findAll(".hidden-secondary .conversation-shortcuts")).toHaveLength(
      0,
    );
  } finally {
    w.unmount();
  }
});

it("closes floating and pinned history, and resets pinning when reopened", async () => {
  const w = await setup();
  try {
    for (const pin of [false, true]) {
      await w.get('[data-action="history-conversations"]').trigger("click");
      if (pin) await w.get('[aria-label="固定历史会话"]').trigger("click");
      await w.get('[aria-label="关闭历史会话"]').trigger("click");
      expect(w.find(".conversation-history-panel").exists()).toBe(false);
      expect(w.get(".workspace-content").classes()).not.toContain(
        "history-reserved",
      );
    }
    await w.get('[data-action="history-conversations"]').trigger("click");
    expect(w.get(".conversation-history-panel").classes()).not.toContain(
      "history-pinned",
    );
  } finally {
    w.unmount();
  }
});

it("keeps secondary navigation with its current page after search and settings actions", async () => {
  const w = await setup("settings");
  try {
    await w.getComponent(AccountMenu).get("button").trigger("click");
    await w.get(".sidebar-cycle").trigger("click");
    await w.get(".sidebar-cycle").trigger("click");
    w.getComponent(WorkspaceSearch).vm.$emit("select", {
      kind: "section",
      page: "system-settings",
      id: "network",
    });
    await w.setProps({ page: "system-settings" });
    await flushPromises();
    expect(w.get(".hidden-secondary").text()).not.toContain("账号与组织");
    expect(w.get(".hidden-secondary").text()).toContain("网络");
    await w.getComponent(AccountMenu).vm.$emit("open");
    await w.setProps({ page: "settings" });
    await w.getComponent(Settings).vm.$emit("assistant");
    await w.setProps({ page: "assistant" });
    expect(w.get(".hidden-secondary").text()).not.toContain("账号与组织");
    expect(w.get(".hidden-secondary").text()).toContain("新会话");
    await w.getComponent(AccountMenu).vm.$emit("open");
    await w.setProps({ page: "settings" });
    await w
      .getComponent(Settings)
      .findAll("button")
      .find((b) => b.text() === "执行服务记录")!
      .trigger("click");
    expect(w.emitted("navigate")?.at(-1)).toEqual(["tasks"]);
    await w.setProps({ page: "tasks" });
    expect(
      w.find(".hidden-secondary .agent-settings-navigation").exists(),
    ).toBe(false);
  } finally {
    w.unmount();
  }
});

it("opens the registered diagnostics section from the tools help action", async () => {
  const w = await setup("device-tools");
  try {
    await w
      .getComponent(ToolsPage)
      .get(".organization-tools button")
      .trigger("click");
    expect(w.emitted("navigate")?.at(-1)).toEqual(["settings"]);
    await w.setProps({ page: "settings" });
    expect(navigation(w.getComponent(Settings)).section).toBe("diagnostics");
  } finally {
    w.unmount();
  }
});
