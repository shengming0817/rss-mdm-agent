import { mount, flushPromises } from "@vue/test-utils";
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
        snapshot: vi
          .fn()
          .mockResolvedValue({ ...snapshot(), available: [resourceOffer()] }),
        execute: vi.fn(),
        cancel: vi.fn(),
      },
    },
  });
  const resource = w.getComponent(SelfService);
  await flushPromises();
  await resource.get('[data-action="resource-details"]').trigger("click");
  await resource.get('[data-action="ask-ai"]').trigger("click");
  await flushPromises();
  const drawer = w.get('dialog[aria-label="资源上下文 AI"]');
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
  await drawer.trigger("cancel");
  await flushPromises();
  expect(w.find('dialog[aria-label="资源上下文 AI"]').exists()).toBe(false);
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
        snapshot: vi.fn().mockResolvedValue(snapshot()),
        execute: vi.fn(),
        cancel: vi.fn(),
      },
    },
  });
  try {
    await flushPromises();
    expect(w.get(".navigation-brand").text()).toContain("RSS");
    expect(w.get(".shell-header").text()).toContain("新对话");
    expect(document.querySelectorAll(".shell-header h1")).toHaveLength(1);
    const assistant = w.getComponent(Assistant);
    expect(assistant.props("presentation")).toBe("main");
    expect(assistant.text()).toContain("今天，想完成什么？");
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
        snapshot: vi
          .fn()
          .mockResolvedValue({ ...snapshot(), available: [resourceOffer()] }),
        execute,
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
    const panel = w.get('dialog[aria-label="资源上下文 AI"]');
    await panel.get("select").setValue("new");
    await panel.get('[data-action="choose-conversation"]').trigger("click");
    await flushPromises();
    expect(view.props("presentation")).toBe("context");
    expect(view.text()).not.toContain("今天，想完成什么？");
    expect(view.get("textarea").element).toBe(input);
    await view.get('[data-action="suggest-prompt"]').trigger("click");
    expect(c.draft.value).toContain("资源");
    expect(prompt).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
    await panel.trigger("cancel");
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
