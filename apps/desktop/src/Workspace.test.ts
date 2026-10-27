import { policyFixturePort } from "../../../tests/self-service/support";
import { mount, flushPromises } from "@vue/test-utils";
import { it, expect, vi } from "vitest";
import Workspace from "./Workspace.vue";
import Assistant from "./assistant/Assistant.vue";
import Tasks from "./task-center/TaskCenter.vue";
import {
  taskPort,
  taskDetail,
  taskPage,
} from "../../../tests/self-service/support";
import { createHostSettings } from "./settings/controller";

it("attaches the current page to one shared assistant and preserves question text when the sidebar closes", async () => {
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "software-management",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        offers: vi.fn(async () => []),
        execute: vi.fn(),
        confirm: vi.fn(),
      },
    },
  });
  const c = w.getComponent(Assistant).props("controller");
  try {
    await flushPromises();
    c.draft.value = "我的问题";
    await w.get(".workspace-assistant-entry").trigger("click");
    await flushPromises();
    expect(w.findAllComponents(Assistant)).toHaveLength(1);
    expect(c.context.value?.scope).toBe("page");
    expect(c.context.value?.path[0]).toBe("软件管理");
    await w
      .get('.page-assistant-panel [data-action="history-conversations"]')
      .trigger("click");
    await flushPromises();
    await w.get('[aria-label="固定历史会话"]').trigger("click");
    await w.get('[aria-label="关闭电脑助手侧栏"]').trigger("click");
    await flushPromises();
    expect(w.find(".page-assistant-panel").exists()).toBe(false);
    expect(c.draft.value).toBe("我的问题");
    expect(c.context.value).toBeDefined();
    await w.get(".workspace-assistant-entry").trigger("click");
    await flushPromises();
    expect(w.find(".conversation-history-panel").exists()).toBe(false);
    expect(c.draft.value).toBe("我的问题");
    await w.get('[aria-label="关闭电脑助手侧栏"]').trigger("click");
    await w.setProps({ page: "assistant" });
    expect(w.getComponent(Assistant).props("controller")).toBe(c);
  } finally {
    w.unmount();
  }
  expect(c.state.drafts.size).toBe(0);
});

it("uses one content header, a sidebar brand and a main welcome without remounting the composer", async () => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 1100,
    height: 760,
  } as DOMRect);
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "assistant",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        offers: vi.fn(async () => []),
        execute: vi.fn(),
        confirm: vi.fn(),
      },
    },
  });
  try {
    await flushPromises();
    expect(w.text()).not.toContain("你的 AI 与设备工作区");
    expect(w.get(".shell-header").text()).toContain("新对话");
    expect(document.querySelectorAll(".shell-header h1")).toHaveLength(1);
    const assistant = w.getComponent(Assistant);
    expect(assistant.props("presentation")).toBe("main");
    expect(assistant.text()).toContain("有什么可以帮你？");
    const c = assistant.props("controller");
    const input = assistant.get("textarea").element;
    const prompt = vi.spyOn(c, "prompt");
    const create = vi.spyOn(c, "create");
    await assistant.get('[data-action="suggest-prompt"]').trigger("click");
    expect(c.draft.value).not.toBe("");
    expect(prompt).not.toHaveBeenCalled();
    expect(create).not.toHaveBeenCalled();
    c.draft.value = "已有草稿";
    await flushPromises();
    expect(
      assistant.get('[data-action="suggest-prompt"]').attributes("disabled"),
    ).toBeDefined();
    await assistant.get('[data-action="suggest-prompt"]').trigger("click");
    expect(c.draft.value).toBe("已有草稿");
    await w.setProps({ page: "software-management" });
    await w.setProps({ page: "assistant" });
    expect(w.getComponent(Assistant).get("textarea").element).toBe(input);
  } finally {
    w.unmount();
  }
});

it("keeps one composer while the page assistant stages a suggestion and returns to the main conversation", async () => {
  const execute = vi.fn();
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "software-management",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        offers: vi.fn(async () => []),
        execute,
        confirm: vi.fn(),
      },
    },
  });
  try {
    await flushPromises();
    const view = w.getComponent(Assistant),
      c = view.props("controller"),
      input = view.get("textarea").element;
    const prompt = vi.spyOn(c, "prompt");
    await w.get(".workspace-assistant-entry").trigger("click");
    await flushPromises();
    expect(view.props("presentation")).toBe("context");
    expect(view.get("textarea").element).toBe(input);
    await view.get('[data-action="suggest-prompt"]').trigger("click");
    expect(c.draft.value).toContain("页面");
    expect(prompt).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
    await w.get('[aria-label="在 AI 页面继续"]').trigger("click");
    await w.setProps({ page: "assistant" });
    await flushPromises();
    expect(view.props("presentation")).toBe("main");
    expect(view.get("textarea").element).toBe(input);
    expect(c.context.value?.path[0]).toBe("软件管理");
  } finally {
    w.unmount();
  }
});

it("opens system settings separately from application settings and preserves assistant drafts", async () => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 1100,
    height: 760,
  } as DOMRect);
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "assistant",
      host: createHostSettings(undefined),
    },
  });
  await flushPromises();
  const assistant = w.getComponent(Assistant).props("controller");
  assistant.draft.value = "保留这个问题";
  const entry = w
    .findAll("button")
    .find((button) => button.text() === "系统设置")!;
  await entry.trigger("click");
  expect(w.emitted("navigate")?.at(-1)).toEqual(["system-settings"]);
  await w.setProps({ page: "system-settings" });
  expect(w.get(".workspace-breadcrumb").text()).toBe("系统设置");
  expect(w.get(".resource-content").isVisible()).toBe(false);
  expect(w.get(".system-settings").isVisible()).toBe(true);
  await w.get(".workspace-assistant-entry").trigger("click");
  await flushPromises();
  expect(w.find(".page-assistant-panel").exists()).toBe(true);
  expect(w.emitted("navigate")?.at(-1)).toEqual(["system-settings"]);

  await w.setProps({ page: "assistant" });
  expect(w.getComponent(Assistant).props("controller")).toBe(assistant);
  expect(assistant.draft.value).toBe("保留这个问题");
  w.unmount();
});

it("keeps navigation and account access after login failure without displaying authenticated content", async () => {
  const w = mount(Workspace, {
    props: {
      ready: false,
      busy: false,
      page: "settings",
      host: createHostSettings(undefined),
    },
  });
  try {
    await flushPromises();
    expect(w.find(".workspace-primary-navigation").exists()).toBe(true);
    expect(w.find(".workspace-settings-footer").exists()).toBe(true);
    expect(w.findComponent(Tasks).exists()).toBe(false);
    expect(w.findComponent(Assistant).exists()).toBe(false);
    expect(w.get(".workspace-page-header h1").text()).toBe("设置");
  } finally {
    w.unmount();
  }
});

it("refreshes FileVault custody on settings entry and while settings stay visible", async () => {
  vi.useFakeTimers();
  const read = vi.fn().mockResolvedValue({
    attributes: {
      computerName: "Fixture Mac",
      ipAddresses: [],
      loginUser: null,
    },
    filevault: "pending",
  });
  const w = mount(Workspace, {
    props: {
      ready: false,
      busy: false,
      page: "home",
      fixture: true,
      host: createHostSettings(undefined),
      devicePort: { read },
    },
  });
  try {
    await flushPromises();
    const initial = read.mock.calls.length;
    await w.setProps({ page: "settings" });
    await flushPromises();
    expect(read.mock.calls.length).toBeGreaterThan(initial);
    expect(w.text()).toContain("上传待确认");
    read.mockResolvedValue({
      attributes: {
        computerName: "Fixture Mac",
        ipAddresses: [],
        loginUser: null,
      },
      filevault: "escrowedUnverified",
    });
    await vi.advanceTimersByTimeAsync(30000);
    await flushPromises();
    expect(w.text()).toContain("已托管，恢复有效性尚未验证");
    const visible = read.mock.calls.length;
    vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    await vi.advanceTimersByTimeAsync(30000);
    expect(read.mock.calls.length).toBe(visible);
  } finally {
    w.unmount();
    vi.restoreAllMocks();
    vi.useRealTimers();
  }
});

it("stops shared task reads while native identity is transitioning without controlling accepted work", async () => {
  vi.useFakeTimers();
  const tasks = taskPort();
  tasks.list = vi.fn(tasks.list);
  tasks.control = vi.fn(tasks.control);
  const w = mount(Workspace, {
    props: {
      ready: true,
      busy: true,
      page: "home",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
      localPorts: {
        tasks,
        data: null,
        software: null,
        settings: null,
        network: null,
      },
    },
  });
  try {
    await flushPromises();
    expect(tasks.list).not.toHaveBeenCalled();
    await w.setProps({ busy: false });
    await flushPromises();
    expect(tasks.list).toHaveBeenCalledTimes(1);
    await w.setProps({ busy: true });
    await vi.advanceTimersByTimeAsync(60000);
    expect(tasks.list).toHaveBeenCalledTimes(1);
    expect(tasks.control).not.toHaveBeenCalled();
  } finally {
    w.unmount();
    vi.useRealTimers();
  }
});

it("loads retained Home facts on first visit and preserves them across navigation", async () => {
  const read: import("./system-settings/controller").SettingsPort = async (
    operation,
  ) => {
    if (operation.kind !== "snapshot")
      throw Error("only retained reads expected");
    return {
      writeAllowed: false,
      request: null,
      digest: null,
      finished: true,
      effect: "observed",
      keepBeforeUnixMs: null,
      reason: null,
      snapshot: {
        section: operation.section,
        revision: "a".repeat(64),
        collectedAtUnixMs: Date.now(),
        source: "native fixture",
        targets: [],
      },
    };
  };
  const settings = vi.fn(read);
  const w = mount(Workspace, {
    props: {
      ready: true,
      busy: false,
      page: "settings",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
      localPorts: {
        tasks: taskPort(),
        data: null,
        software: null,
        settings,
        network: null,
      },
    },
  });
  try {
    await flushPromises();
    expect(settings).not.toHaveBeenCalled();
    await w.setProps({ page: "home" });
    await flushPromises();
    expect(
      settings.mock.calls.map(([operation]) =>
        operation.kind === "snapshot" ? operation.section : operation.kind,
      ),
    ).toEqual(["network", "storage", "power"]);
    await w.setProps({ page: "settings" });
    await w.setProps({ page: "home" });
    await flushPromises();
    expect(settings).toHaveBeenCalledTimes(3);
  } finally {
    w.unmount();
  }
});

it("starts one shared task read for the visible software consumer and keeps device reads hidden", async () => {
  const tasks = taskPort();
  tasks.list = vi.fn(tasks.list);
  const readDevice = vi.fn(async () => ({
    attributes: { computerName: "Mac", ipAddresses: [], loginUser: null },
    filevault: "unavailable" as const,
  }));
  const w = mount(Workspace, {
    props: {
      ready: true,
      busy: false,
      fixture: true,
      page: "software-management",
      host: createHostSettings(undefined),
      devicePort: { read: readDevice },
      selfServicePort: {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
      localPorts: {
        tasks,
        data: null,
        software: null,
        settings: null,
        network: null,
      },
    },
  });
  try {
    await flushPromises();
    expect(tasks.list).toHaveBeenCalledTimes(1);
    expect(readDevice).not.toHaveBeenCalled();
  } finally {
    w.unmount();
  }
});

it("returns from a Home task to its stable reference when a mouse click leaves header focus", async () => {
  const detail = taskDetail("home-original");
  const tasks = {
    ...taskPort(),
    list: vi.fn(async () => taskPage([detail])),
    detail: vi.fn(async () => detail),
    control: vi.fn(),
  };
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      fixture: true,
      page: "home",
      host: createHostSettings(undefined),
      devicePort: {
        read: async () => ({
          attributes: { computerName: "Mac", ipAddresses: [], loginUser: null },
          filevault: "unavailable" as const,
        }),
      },
      selfServicePort: {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
      localPorts: {
        tasks,
        data: null,
        software: null,
        settings: null,
        network: null,
      },
      onNavigate: (page: string) => {
        void w.setProps({ page });
      },
    },
  });
  try {
    await flushPromises();
    const header = w.get("h1").element as HTMLElement;
    header.tabIndex = -1;
    header.focus();
    const origin = w.get(".home-activity button[data-task-ref]");
    await origin.trigger("click");
    await flushPromises();
    expect(w.props("page")).toBe("tasks");
    await w.get(".task-center-detail > button").trigger("click");
    await flushPromises();
    expect(w.props("page")).toBe("home");
    expect(document.activeElement?.getAttribute("data-task-ref")).toBe(
      JSON.stringify(detail.reference),
    );
    expect(tasks.control).not.toHaveBeenCalled();
  } finally {
    w.unmount();
  }
});
