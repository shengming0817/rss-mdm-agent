import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import QuestionCard from "./QuestionCard.vue";
import ExecutionDetails from "./ExecutionDetails.vue";
import Assistant from "./Assistant.vue";
import { createAssistant } from "./controller";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type { ExecutionTaskDetails } from "./execution-types";
import type { InteractionView, SessionView } from "@rss-mdm-agent/ai-client";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
it("renders permission scope from kind even when provider names contradict it", () => {
  const c = createAssistant(undefined, () => "id");
  c.state.permissions.set("p", {
    id: "p",
    request: {
      sessionId: "session-1",
      toolCall: { toolCallId: "tool", title: "tool" },
      options: [
        { optionId: "once", name: "永久允许", kind: "allow_once" },
        { optionId: "always", name: "仅此一次", kind: "allow_always" },
        { optionId: "reject", name: "允许", kind: "reject_once" },
        { optionId: "never", name: "允许", kind: "reject_always" },
      ],
    },
  });
  const wrapper = mount(Assistant, { props: { controller: c } });
  const buttons = wrapper.findAll(".permission-card button");
  expect(buttons.slice(0, 4).map((b) => b.find("strong").text())).toEqual([
    "允许一次",
    "始终允许",
    "拒绝一次",
    "始终拒绝",
  ]);
  expect(buttons[1].text()).toContain("后续匹配请求");
  expect(buttons[1].text()).toContain("提供方说明：仅此一次");
  wrapper.unmount();
  c.dispose();
});
it("labels termination and assessment observations without claiming approval evidence", () => {
  const wrapper = mount(ExecutionDetails, {
    props: {
      details: fixtures.outcomeUnknown as ExecutionTaskDetails,
      now: 1000,
    },
  });
  expect(wrapper.text()).toContain("终止/效果核验证据引用（仅授权可见）");
  expect(wrapper.text()).not.toContain("授权证据引用");
  wrapper.unmount();
});
it("keeps duplicate option labels read-only instead of submitting ambiguous selections", () => {
  const wrapper = mount(QuestionCard, {
    props: {
      enabled: true,
      interaction: {
        status: "pending",
        expiresAtMs: 2000,
        request: {
          questions: [
            {
              question: "Choose?",
              header: "Choice",
              multiSelect: false,
              options: [
                { label: "A", description: "first" },
                { label: "A", description: "second" },
              ],
            },
          ],
        },
      } as unknown as InteractionView,
    },
  });
  expect(wrapper.findAll("button,textarea")).toHaveLength(0);
  expect(wrapper.text()).toContain("first");
  expect(wrapper.text()).toContain("second");
  wrapper.unmount();
});
it("preserves bounded escaped unknown question content without enabling actions", () => {
  const wrapper = mount(QuestionCard, {
    props: {
      enabled: true,
      interaction: {
        status: "pending",
        expiresAtMs: 2000,
        request: {
          question: "真实问题 <img src=x>",
          choices: ["真实选项", "x".repeat(20000)],
        },
      } as unknown as InteractionView,
    },
  });
  expect(wrapper.text()).toContain("真实问题 <img src=x>");
  expect(wrapper.text()).toContain("真实选项");
  expect(wrapper.findAll("img,button,textarea")).toHaveLength(0);
  expect(wrapper.text().length).toBeLessThan(9000);
  wrapper.unmount();
});
it("labels validity as a local clock estimate and does not imply an expired approval remains actionable", async () => {
  const wrapper = mount(ExecutionDetails, {
    props: {
      details: fixtures.approvalRequired as ExecutionTaskDetails,
      now: 500,
    },
  });
  expect(wrapper.text()).toContain("计划尚未生效");
  await wrapper.setProps({ now: 1000 });
  expect(wrapper.text()).toContain("计划在有效期内");
  await wrapper.setProps({ now: 2000 });
  expect(wrapper.text()).toContain("计划已过期");
  expect(wrapper.text()).toContain("执行服务记录：需要管理员批准");
  expect(wrapper.text()).not.toContain("等待管理员批准");
  expect(wrapper.text()).toContain("按本机时间判断");
  wrapper.unmount();
});
it("keeps valid contract timestamps beyond Date's range readable", () => {
  const details = structuredClone(
    fixtures.approvalRequired,
  ) as ExecutionTaskDetails;
  details.plan.validity.expiresAtUnixMs = Number.MAX_SAFE_INTEGER;
  const wrapper = mount(ExecutionDetails, { props: { details, now: 2000 } });
  expect(wrapper.text()).toContain("9007199254740991 Unix ms");
  expect(wrapper.text()).toContain("超出本机日期格式范围");
  expect(wrapper.text()).toContain("计划在有效期内");
  wrapper.unmount();
});
it.each(["prompt", "cancel", "respond"] as const)(
  "keeps %s failure diagnoses alongside running, reconciliation and terminal facts",
  async (kind) => {
    const c = createAssistant(undefined, () => "id"),
      session = fixtureSession();
    c.state.selected = session.namespace.sessionId;
    const v: SessionView = {
      namespace: session.namespace,
      generation: session.binding.generation,
      cursor: 1,
      capabilities: session.capabilities,
      sessionStatus: "active",
      connection: "attached",
      timeline:
        kind === "prompt" ? [{ kind: "prompt", key: "p", sequence: 1 }] : [],
      messages: {},
      tools: {},
      surfaces: {},
      interactions: {},
      commands: {
        p: {
          command: {
            schemaVersion: 4,
            kind: "command",
            sessionId: session.namespace.sessionId,
            commandId: "p",
            expiresAtMs: 1000,
            input:
              kind === "prompt"
                ? { type: "prompt", policy: "queue_next", text: "request" }
                : kind === "cancel"
                  ? {
                      type: "cancel",
                      targetCommandId: "target",
                      generation: session.binding.generation,
                    }
                  : {
                      type: "respond",
                      interactionId: "q",
                      generation: session.binding.generation,
                      answer: { answers: {} },
                    },
          },
          state: "running",
          cancelDispatched: "request_only",
          failure: { code: "unavailable", retry: "reconcile_first" },
        },
      },
    };
    c.state.views.set(session.namespace.sessionId, v);
    const wrapper = mount(Assistant, { props: { controller: c } });
    expect(wrapper.text()).toContain("unavailable");
    expect(wrapper.text()).toContain("先恢复历史并核对");
    expect(wrapper.text()).toContain("不证明模型已终止");
    for (const state of ["reconciliation_required", "terminal"] as const) {
      c.state.views.get(session.namespace.sessionId)!.commands.p.state = state;
      await wrapper.vm.$nextTick();
      expect(wrapper.text()).toContain("unavailable");
    }
    expect(wrapper.text()).not.toContain("等待模型终止事实");
    wrapper.unmount();
    c.dispose();
  },
);
