import { activeStage } from "@rss-mdm-agent/ai-contract";
import { mount, flushPromises } from "@vue/test-utils";
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
  c.state.selected = "session-1";
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
  expect(wrapper.text()).toContain("动作尚未生效");
  await wrapper.setProps({ now: 1000 });
  expect(wrapper.text()).toContain("动作在有效期内");
  await wrapper.setProps({ now: 2000 });
  expect(wrapper.text()).toContain("动作已过期");
  expect(wrapper.text()).toContain("执行服务记录：需要管理员批准");
  expect(wrapper.text()).not.toContain("等待管理员批准");
  expect(wrapper.text()).toContain("按本机时间判断");
  wrapper.unmount();
});
it("keeps valid contract timestamps beyond Date's range readable", () => {
  const details = structuredClone(
    fixtures.approvalRequired,
  ) as ExecutionTaskDetails;
  details.action.validity.expiresAtUnixMs = Number.MAX_SAFE_INTEGER;
  const wrapper = mount(ExecutionDetails, { props: { details, now: 2000 } });
  expect(wrapper.text()).toContain("9007199254740991 Unix ms");
  expect(wrapper.text()).toContain("超出本机日期格式范围");
  expect(wrapper.text()).toContain("动作在有效期内");
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
      generation: activeStage(session).binding.generation,
      cursor: 1,
      capabilities: activeStage(session).capabilities,
      sessionStatus: "active",
      connection: "attached",
      timeline:
        kind === "prompt" ? [{ kind: "prompt", key: "p", sequence: 1 }] : [],
      messages: {},
      tools: {},
      deliveries: {},
      surfaces: {},
      interactions: {},
      commands: {
        p: {
          command: {
            schemaVersion: 6,
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
                      generation: activeStage(session).binding.generation,
                    }
                  : {
                      type: "respond",
                      interactionId: "q",
                      generation: activeStage(session).binding.generation,
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
    expect(wrapper.text()).not.toContain("unavailable");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "会话详情与诊断")!
      .trigger("click");
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

it("renders closed process reasons and quality as actionable Chinese text", () => {
  const groups = {
    end: {
      rejected: "派发前已拒绝",
      exited: "根进程退出",
      cancelled: "已请求取消",
      timedOut: "执行时间额度已耗尽",
      outputLimit: "累计输出额度已耗尽",
      unknown: "结束原因未确认",
    },
    quality: {
      complete: "完整且符合输出契约",
      truncated: "输出已截断",
      failed: "不符合契约",
      partial: "采集尚不完整",
    },
    failureKind: {
      none: "未观察到机制故障",
      denied: "权限或制品校验拒绝",
      unbound: "身份或受控输入未绑定",
      capability: "无法强制所需约束",
      unsupported: "平台不支持所需调用",
      invalidInput: "输入或配置格式无效",
      capacity: "执行资源额度不足",
      conflict: "执行关联发生冲突",
      unavailable: "平台资源不可用",
      runtime: "执行宿主初始化失败",
      spawn: "目标进程启动失败",
      inputDelivery: "受控输入未完整投递",
      capture: "输出读取失败",
      supervision: "进程状态无法可靠核实",
      outputValidation: "输出编码或结构校验失败",
    },
  };
  for (const [field, values] of Object.entries(groups))
    for (const [value, label] of Object.entries(values)) {
      const details = structuredClone(
        fixtures.outcomeUnknown,
      ) as ExecutionTaskDetails;
      details.status.process = {
        finished: true,
        exitCode: 0,
        end: "exited",
        quality: "complete",
        quiescent: false,
        totalOutputBytes: 0,
        failureKind: "none",
        [field]: value,
      };
      const wrapper = mount(ExecutionDetails, {
        props: { details, now: 1000 },
      });
      expect(wrapper.find(".process-facts").text()).toContain(label);
      expect(wrapper.find(".process-facts").text()).not.toContain(value);
      wrapper.unmount();
    }
});
it("keeps stop acknowledgement separate from localized effect assessment", () => {
  for (const [assessment, label] of Object.entries({
    noEffect: "已核实未产生效果",
    satisfied: "已核实效果符合预期",
    notSatisfied: "已核实效果不符合预期",
    unknown: "效果未知，需可信核对",
  }))
    for (const [stop, stopLabel] of Object.entries({
      acknowledged: "停止请求已接收（未确认终止）",
      failed: "停止请求未确认",
    })) {
      const details = structuredClone(
        fixtures.outcomeUnknown,
      ) as ExecutionTaskDetails;
      details.status.assessment = assessment as NonNullable<
        ExecutionTaskDetails["status"]["assessment"]
      >;
      details.status.stopOutcome = stop as NonNullable<
        ExecutionTaskDetails["status"]["stopOutcome"]
      >;
      const wrapper = mount(ExecutionDetails, {
        props: { details, now: 1000 },
      });
      expect(wrapper.text()).toContain(label);
      expect(wrapper.text()).toContain(stopLabel);
      wrapper.unmount();
    }
});

it("shows software recovery diagnostics without treating detection as final success", () => {
  for (const [diagnostic, label] of Object.entries({
    cleanupPending: "资源占用保留",
    cleanupUnverified: "目录归属",
    restartPending: "重启设备",
    detectionUnavailable: "读取权限",
    unrecognizedVersion: "未知软件内容",
    detectionBudgetExceeded: "预算耗尽",
    desiredStateObserved: "不代表后台活动已终止",
  })) {
    const details = structuredClone(fixtures.software) as ExecutionTaskDetails;
    details.status.software = diagnostic as NonNullable<
      ExecutionTaskDetails["status"]["software"]
    >;
    const wrapper = mount(ExecutionDetails, { props: { details, now: 1000 } });
    expect(wrapper.find(".software-diagnostic").text()).toContain(label);
    expect(wrapper.find(".software-operation").text()).toContain("macOS PKG");
    expect(wrapper.find(".software-operation").text()).toContain("安装");
    wrapper.unmount();
  }
});

it("starts with an editable composer, readable history and one live conversation log without an execution panel", () => {
  const c = createAssistant(undefined, () => "id");
  const session = fixtureSession();
  c.state.sessions.set(session.namespace.sessionId, {
    namespace: session.namespace,
    status: "active",
    title: "检查网络连接",
    lastActivityAtMs: 1,
  });
  const wrapper = mount(Assistant, { props: { controller: c } });
  expect(wrapper.get("textarea").attributes("disabled")).toBeUndefined();
  expect(wrapper.text()).toContain("检查网络连接");
  expect(wrapper.text()).not.toContain(session.namespace.sessionId);
  expect(wrapper.findAll('[role="log"]')).toHaveLength(1);
  expect(wrapper.find(".assistant-execution").exists()).toBe(false);
  expect(wrapper.find(".execution-activity").exists()).toBe(false);
  expect(wrapper.text()).not.toContain("收起输入");
  wrapper.unmount();
  c.dispose();
});

it.each([
  ["authentication_required", "前往连接设置"],
  ["connection_required", "前往连接设置"],
  ["context_unavailable", "选择连接与新上下文"],
])(
  "routes %s to its recovery action even with a detached view",
  async (code, action) => {
    const c = createAssistant(undefined, () => "id");
    c.state.connection = "connected";
    c.state.selected = "session-1";
    c.state.errors.set("session-1", code);
    const wrapper = mount(Assistant, { props: { controller: c } });
    const notice = wrapper.get(".conversation-notice");
    expect(notice.text()).toContain(action);
    await notice.get("button").trigger("click");
    if (code === "context_unavailable")
      expect(wrapper.get(".connection-menu").attributes("open")).toBeDefined();
    else expect(wrapper.emitted("settings")).toHaveLength(1);
    wrapper.unmount();
    c.dispose();
  },
);
it("keeps the specific connection failure actionable", () => {
  const c = createAssistant(undefined, () => "id");
  c.state.error = "authentication_required";
  const wrapper = mount(Assistant, { props: { controller: c } });
  expect(wrapper.get(".conversation-notice").text()).toContain("认证不可用");
  expect(wrapper.get(".conversation-notice button").text()).toBe(
    "前往连接设置",
  );
  wrapper.unmount();
  c.dispose();
});
it("follows newly requested permission only when already following the conversation", async () => {
  const c = createAssistant(undefined, () => "id");
  c.state.selected = "session-1";
  const wrapper = mount(Assistant, { props: { controller: c } });
  const timeline = wrapper.get(".assistant-timeline");
  Object.defineProperty(timeline.element, "scrollHeight", { value: 1000 });
  Object.defineProperty(timeline.element, "clientHeight", { value: 200 });
  const permission = (id: string) => ({
    id,
    request: {
      sessionId: "session-1",
      toolCall: { toolCallId: id, title: "permission" },
      options: [],
    },
  });
  c.state.permissions.set("p1", permission("p1"));
  await flushPromises();
  expect(timeline.element.scrollTop).toBe(1000);
  timeline.element.scrollTop = 100;
  await timeline.trigger("scroll");
  c.state.permissions.set("p2", permission("p2"));
  await flushPromises();
  expect(timeline.element.scrollTop).toBe(100);
  wrapper.unmount();
  c.dispose();
});
