import { activeStage } from "@rss-mdm-agent/ai-contract";
import { DOMWrapper, mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import App from "./App.vue";
import AccountMenu from "./workspace/AccountMenu.vue";
import Assistant from "./assistant/Assistant.vue";
import type { AssistantController } from "./assistant/controller";
import type { SessionView } from "@rss-mdm-agent/ai-client";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
it("replaces the old sample with an explicitly read-only browser preview", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockRejectedValue(new Error("unexpected network"));
  const wrapper = mount(App, { attachTo: document.body });
  expect(wrapper.text()).toContain("浏览器只读预览");
  expect(
    wrapper
      .findAll(".workspace-rail .ui-navigation > ol > li > button")
      .some((e) => e.text() === "软件中心"),
  ).toBe(false);
  expect(wrapper.text()).toContain("工具中心");
  const assistant = wrapper
    .findAll("button")
    .find((button) => button.text() === "AI 助手")!;
  expect(assistant).toBeDefined();
  await assistant.trigger("click");
  expect(wrapper.text()).toContain("AI 服务未连接");
  expect(wrapper.text()).not.toContain("消息与输入");
  expect(fetch).not.toHaveBeenCalled();
  wrapper.unmount();
});

it("expires navigation badge, background entry and question card together without a server update", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(100);
  const wrapper = mount(App, { attachTo: document.body });
  try {
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "AI 助手")!
      .trigger("click");
    const c = wrapper
      .findComponent(Assistant)
      .props("controller") as AssistantController;
    const session = fixtureSession();
    c.state.connection = "connected";
    c.state.selected = "selected";
    for (const id of ["selected", "background"]) {
      const v: SessionView = {
        namespace: { ...session.namespace, sessionId: id },
        generation: activeStage(session).binding.generation,
        cursor: 1,
        capabilities: {
          ...activeStage(session).capabilities,
          structuredQuestion: "supported",
        },
        sessionStatus: "active",
        connection: "attached",
        commands: {},
        messages: {},
        tools: {},
        deliveries: {},
        surfaces: {},
        timeline: [{ kind: "interaction", key: "q", sequence: 1 }],
        interactions: {
          q: {
            commandId: "p",
            generation: activeStage(session).binding.generation,
            status: "pending",
            expiresAtMs: 1000,
            callbackLifetime: "generation_bound",
            request: {
              questions: [
                {
                  question: "Choose?",
                  header: "Choice",
                  multiSelect: false,
                  options: [
                    { label: "A", description: "a" },
                    { label: "B", description: "b" },
                  ],
                },
              ],
            },
          },
        },
      };
      c.state.views.set(id, v);
    }
    await wrapper.vm.$nextTick();
    await wrapper.get('[aria-label="打开主导航"]').trigger("click");
    expect(wrapper.find('[aria-label="待回应 2"]').exists()).toBe(true);
    await new DOMWrapper(
      document.querySelector('[role=\"dialog\"][aria-label=\"主导航\"]')!,
    )
      .get('[aria-label^="关闭"]')
      .trigger("click");
    expect(wrapper.find(".assistant .notice").text()).toContain("新对话");
    expect(
      wrapper.find(".question-card fieldset").attributes("disabled"),
    ).toBeUndefined();
    await vi.advanceTimersByTimeAsync(1001);
    await wrapper.get('[aria-label="打开主导航"]').trigger("click");
    expect(
      wrapper
        .findAll("button")
        .find((b) => b.text().startsWith("AI 助手"))!
        .text(),
    ).toBe("AI 助手");
    await new DOMWrapper(
      document.querySelector('[role=\"dialog\"][aria-label=\"主导航\"]')!,
    )
      .get('[aria-label^="关闭"]')
      .trigger("click");
    expect(wrapper.find(".assistant .notice").exists()).toBe(false);
    expect(
      wrapper.find(".question-card fieldset").attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.find(".question-card").text()).toContain("已过期");
  } finally {
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});

it("keeps settings and diagnostics reachable without an AI connection", async () => {
  const wrapper = mount(App, { attachTo: document.body });
  await wrapper.getComponent(AccountMenu).get("button").trigger("click");
  await wrapper
    .get(".workspace-rail .agent-settings-navigation")
    .findAll("button")
    .find((b) => b.text() === "运行与诊断")!
    .trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("运行与诊断");
  expect(wrapper.text()).toContain("非登录");
  expect(wrapper.text()).toContain("隐私");
  expect(wrapper.find('input[type="password"]').exists()).toBe(false);
  wrapper.unmount();
});

it("shares the home draft with the assistant without sending or replacing it", async () => {
  const bounds = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockReturnValue(new DOMRect(0, 0, 1100, 760));
  const wrapper = mount(App, { attachTo: document.body });
  const navigate = async (label: string) => {
    await wrapper
      .findAll("button")
      .find((button) => button.text() === label)!
      .trigger("click");
  };
  try {
    await navigate("主页");
    expect(wrapper.find(".computer-home").isVisible()).toBe(true);
    await wrapper
      .get(".computer-home .home-composer textarea")
      .setValue("检查应用连接");
    expect(
      wrapper.get(".hero-ai-prompts button").attributes("disabled"),
    ).toBeDefined();
    await wrapper.get(".home-composer").trigger("submit");
    const assistant = wrapper
      .findComponent(Assistant)
      .props("controller") as AssistantController;
    expect(assistant.draft.value).toBe("检查应用连接");
    expect(assistant.state.sessions.size).toBe(0);
    await navigate("主页");
    expect(
      (
        wrapper.get(".computer-home .home-composer textarea")
          .element as HTMLTextAreaElement
      ).value,
    ).toBe("检查应用连接");
    await wrapper.findAll(".home-stats > button")[3].trigger("click");
    expect(wrapper.find(".computer-home").exists()).toBe(false);
    expect(wrapper.get(".software-reference").isVisible()).toBe(true);
  } finally {
    wrapper.unmount();
    bounds.mockRestore();
  }
});
