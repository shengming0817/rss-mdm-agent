import { policyFixturePort } from "../../../tests/self-service/support";
import { DOMWrapper, mount, flushPromises } from "@vue/test-utils";
import { it, expect, vi } from "vitest";
import Workspace from "./Workspace.vue";
import Assistant from "./assistant/Assistant.vue";
import SelfService from "./self-service/SelfService.vue";
import { resourceOffer, snapshot } from "../../../tests/self-service/support";
import { createHostSettings } from "./settings/controller";

it("requires explicit conversation selection, keeps one view and preserves staged context on close", async () => {
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "software",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        snapshot: vi
          .fn()
          .mockResolvedValue({ ...snapshot(), available: [resourceOffer()] }),
        execute: vi.fn(),
        confirm: vi.fn(),
        cancel: vi.fn(),
      },
    },
  });
  const resource = w.getComponent(SelfService);
  await flushPromises();
  const resourceTrigger = resource.get('[data-action="resource-details"]');
  await resourceTrigger.trigger("click");
  await flushPromises();
  expect(document.activeElement).toBe(
    resource.get(".resource-details h3").element,
  );
  await resource.get(".resource-details button").trigger("click");
  await flushPromises();
  expect(document.activeElement).toBe(resourceTrigger.element);
  await resourceTrigger.trigger("click");
  await flushPromises();
  await resource.get('[data-action="ask-ai"]').trigger("click");
  await flushPromises();
  const drawer = new DOMWrapper(
    document.querySelector('[role=\"dialog\"][aria-label=\"资源上下文 AI\"]')!,
  );
  expect(drawer.text()).toContain("办公套件");
  const c = w.getComponent(Assistant).props("controller");
  expect(c.context.value).toBeUndefined();
  expect(
    drawer.get('[data-action="choose-conversation"]').attributes("disabled"),
  ).toBeDefined();
  await drawer.get("select").setValue("new");
  await drawer.get('[data-action="choose-conversation"]').trigger("click");
  await flushPromises();
  expect(w.findAllComponents(Assistant)).toHaveLength(1);
  expect(c.context.value!.path).toEqual([
    "软件中心",
    "未提供分类",
    "办公套件",
    "软件详情",
  ]);
  c.draft.value = "我的问题";
  await drawer.get('[aria-label^="关闭"]').trigger("click");
  await flushPromises();
  expect(
    !!document.querySelector('[role="dialog"][aria-label="资源上下文 AI"]'),
  ).toBe(false);
  expect(c.draft.value).toBe("我的问题");
  expect(c.context.value).toBeDefined();
  await w.setProps({ page: "assistant" });
  expect(w.getComponent(Assistant).props("controller")).toBe(c);
  w.unmount();
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
        snapshot: vi.fn().mockResolvedValue(snapshot()),
        execute: vi.fn(),
        confirm: vi.fn(),
        cancel: vi.fn(),
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
    await w.setProps({ page: "software" });
    await w.setProps({ page: "assistant" });
    expect(w.getComponent(Assistant).get("textarea").element).toBe(input);
  } finally {
    w.unmount();
  }
});

it("changes the same assistant to compact resource presentation and stages suggestions only in its draft", async () => {
  const execute = vi.fn();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 1100,
    height: 760,
  } as DOMRect);
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "software",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        snapshot: vi
          .fn()
          .mockResolvedValue({ ...snapshot(), available: [resourceOffer()] }),
        execute,
        confirm: vi.fn(),
        cancel: vi.fn(),
      },
    },
  });
  try {
    await flushPromises();
    const view = w.getComponent(Assistant);
    const c = view.props("controller");
    const input = view.get("textarea").element;
    const prompt = vi.spyOn(c, "prompt");
    await w.get('[data-action="resource-details"]').trigger("click");
    expect(execute).not.toHaveBeenCalled();
    await w.get('[data-action="ask-ai"]').trigger("click");
    await flushPromises();
    const panel = new DOMWrapper(
      document.querySelector(
        '[role=\"dialog\"][aria-label=\"资源上下文 AI\"]',
      )!,
    );
    await panel.get("select").setValue("new");
    await panel.get('[data-action="choose-conversation"]').trigger("click");
    await flushPromises();
    expect(view.props("presentation")).toBe("context");
    expect(view.text()).not.toContain("有什么可以帮你？");
    expect(view.get("textarea").element).toBe(input);
    await view.get('[data-action="suggest-prompt"]').trigger("click");
    expect(c.draft.value).toContain("资源");
    expect(prompt).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
    await panel.get('[aria-label^="关闭"]').trigger("click");
    await flushPromises();
    await w.setProps({ page: "assistant" });
    expect(view.props("presentation")).toBe("main");
    expect(view.get("textarea").element).toBe(input);
    expect(c.context.value?.path[2]).toBe("办公套件");
    expect(c.draft.value).toContain("资源");
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

  expect(w.getComponent(SelfService).props("controller").state.page).not.toBe(
    "system-settings",
  );
  await w.setProps({ page: "assistant" });
  expect(w.getComponent(Assistant).props("controller")).toBe(assistant);
  expect(assistant.draft.value).toBe("保留这个问题");
  w.unmount();
});
