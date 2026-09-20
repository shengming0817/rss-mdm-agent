import { activeStage } from "@rss-mdm-agent/ai-contract";
import { mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import App from "./App.vue";
import Assistant from "./assistant/Assistant.vue";
import type { AssistantController } from "./assistant/controller";
import type { SessionView } from "@rss-mdm-agent/ai-client";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
it("replaces the old sample with an explicitly read-only browser preview", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockRejectedValue(new Error("unexpected network"));
  const wrapper = mount(App);
  expect(wrapper.text()).toContain("浏览器只读预览");
  expect(wrapper.text()).toContain("软件中心");
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
  const wrapper = mount(App);
  try {
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
    expect(wrapper.text()).toContain("AI 助手（待回应 2）");
    expect(wrapper.find(".assistant .notice").text()).toContain("background");
    expect(
      wrapper.find(".question-card fieldset").attributes("disabled"),
    ).toBeUndefined();
    await vi.advanceTimersByTimeAsync(1001);
    expect(wrapper.text()).not.toContain("待回应");
    expect(wrapper.find(".assistant .notice").exists()).toBe(false);
    expect(
      wrapper.find(".question-card fieldset").attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.find(".question-card").text()).toContain("已过期");
  } finally {
    wrapper.unmount();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});
