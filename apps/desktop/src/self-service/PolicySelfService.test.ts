import { mount, flushPromises } from "@vue/test-utils";
import { expect, test, vi } from "vitest";
import PolicySelfService from "./PolicySelfService.vue";
import PolicyPreparation from "./PolicyPreparation.vue";
import { currentUser } from "../test-users";
import type { BackendRequest, SelfServicePort, SelfServiceTask } from "./types";
const record = (id: string): BackendRequest => ({
  action: {
    kind: "policy",
    locator: { after: null },
    choice: {
      request: id,
      policyId: "policy",
      policyRevision: 1,
      policyVersion: "version",
      parameters: { name: "Acme" },
    },
    directory: {
      subject: {
        kind: "user",
        user: {
          tenantId: "tenant",
          instanceId: "instance",
          principalId: "principal",
        },
      },
      policyId: "policy",
      policyRevision: 1,
      policyVersion: "version",
      task: {
        kind: "script",
        runAs: "system",
        resource: { id: "resource", version: "1", variant: "macos" },
        parameterSchema: { type: "object" },
        parameterSources: { name: { kind: "input" } },
      },
      selfService: {
        published: true,
        allowAi: true,
        riskLevel: 1,
        presentation: {
          displayName: "Policy",
          description: "Read selected state",
          category: "Tools",
          keywords: [],
          prerequisites: "Current login",
          sideEffects: "Read only",
        },
      },
    },
    access: {
      kind: "user",
      sessionBinding: { id: "binding", generation: "generation" },
    },
    expiresAtUnixMs: Date.now() + 60000,
    runId: "run",
    offer: null,
  },
  trigger: {
    kind: "human",
    osSession: {
      device: "device",
      account: { platform: "macos", subject: "501" },
      session: "login",
    },
  },
  risk: null,
  confirmation: null,
  revision: 2,
  state: "submitting",
  failure: null,
});
test.each([
  {
    runAs: "system",
    target: { kind: "device" },
    expected: ["系统账户", "本机设备"],
  },
  {
    runAs: "logged_in_user",
    target: {
      kind: "user",
      identity: "target-501",
      sessionId: "target-login-42",
    },
    expected: [
      "本机登录用户",
      "目标用户 target-501",
      "登录会话 target-login-42",
    ],
  },
] as const)(
  "human and original-request confirmations display execution context: $runAs",
  async ({ runAs, target, expected }) => {
    const original = record("original-request");
    if (original.action.kind !== "policy") throw Error("policy fixture");
    const item = original.action.directory;
    if (item.task.kind !== "script") throw Error("script fixture");
    item.task = {
      kind: "software",
      runAs,
      target,
      resource: item.task.resource,
    } satisfies SelfServiceTask;
    original.state = "awaitingConfirmation";
    original.action.choice.parameters = {};
    const port = {
      policy_directory: async () => ({
        items: [item],
        nextCursor: null,
        asOf: 1,
      }),
    } as unknown as SelfServicePort;
    const human = mount(PolicySelfService, { props: { port, records: [] } });
    const preparation = mount(PolicyPreparation, {
      props: { record: original },
    });
    try {
      await flushPromises();
      await human
        .findAll("button")
        .find((b) => b.text() === "Policy")!
        .trigger("click");
      await human.get("form").trigger("submit");
      const confirmation = human.get('[aria-label="人工执行确认"]');
      for (const text of expected) {
        expect(confirmation.text()).toContain(text);
        expect(preparation.text()).toContain(text);
      }
      expect(confirmation.text()).toContain("准入主体");
      expect(preparation.text()).toContain("准入主体");
      expect(preparation.text()).toContain("principal");
      expect(preparation.text()).not.toContain("最终参数");
      expect(preparation.get("button").text()).toBe("查询原请求与结果");
    } finally {
      human.unmount();
      preparation.unmount();
    }
  },
);
test("remounted history queries its original UUID and does not invoke again", async () => {
  const original = "00000000-0000-0000-0000-000000000001";
  const policy_status = vi.fn(async () => ({
    kind: "run",
    value: {
      requestId: original,
      runId: "run",
      state: { execution: "succeeded", cancellation: "none" },
      result: {
        kind: "script",
        evidence: {
          collectedAt: 1,
          quality: "complete",
          schemaValid: true,
          budgetValid: true,
          trusted: true,
          output: { message: "historical-result" },
          outputReference: null,
        },
      },
    },
  }));
  const policy_invoke = vi.fn();
  const port = {
    policy_directory: async () => ({ items: [], nextCursor: null, asOf: 1 }),
    policy_status,
    policy_invoke,
  } as unknown as SelfServicePort;
  const wrapper = mount(PolicySelfService, {
    props: { port, records: [record(original)] },
  });
  await flushPromises();
  const button = wrapper
    .findAll("button")
    .find((b) => b.text() === "查询原请求与结果")!;
  await button.trigger("click");
  await flushPromises();
  expect(policy_status).toHaveBeenCalledWith({ requestId: original });
  expect(wrapper.text()).toContain("historical-result");
  expect(wrapper.text()).toContain(original);
  expect(policy_invoke).not.toHaveBeenCalled();
  expect(wrapper.text()).not.toContain("目标设备 / OS 账号");
});

test("enum and length constraints prevent invalid confirmation; absent restores editing", async () => {
  const item = (
    record("fixture").action as Extract<
      BackendRequest["action"],
      { kind: "policy" }
    >
  ).directory;
  if (item.task.kind !== "script") throw Error("script");
  item.task.parameterSchema = {
    type: "object",
    properties: {
      mode: { type: "string", enum: ["safe", "full"] },
      name: { type: "string", minLength: 2, maxLength: 4 },
    },
    required: ["mode", "name"],
    additionalProperties: false,
  };
  item.task.parameterSources = {
    mode: { kind: "input" },
    name: { kind: "input" },
  };
  const policy_invoke = vi
    .fn()
    .mockRejectedValue(new Error("rejected before creation"));
  const port = {
    policy_directory: async () => ({
      items: [item],
      nextCursor: null,
      asOf: 1,
    }),
    policy_invoke,
    policy_status: async () => ({ kind: "absent" }),
  } as unknown as SelfServicePort;
  const wrapper = mount(PolicySelfService, { props: { port, records: [] } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "Policy")!
    .trigger("click");
  expect(wrapper.findAll("option").map((o) => o.text())).toEqual([
    "请选择",
    "safe",
    "full",
  ]);
  await wrapper.find("select").setValue(JSON.stringify("safe"));
  await wrapper.find("input").setValue("x");
  await wrapper.find("form").trigger("submit");
  expect(wrapper.find('[aria-label="人工执行确认"]').exists()).toBe(false);
  expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  await wrapper.find("input").setValue("Acme");
  await wrapper.find("form").trigger("submit");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "确认并执行")!
    .trigger("click");
  await flushPromises();
  expect(policy_invoke.mock.calls[0][0].choice.parameters).toEqual({
    mode: "safe",
    name: "Acme",
  });
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "查询原请求")!
    .trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("原请求未创建");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "修改参数")!
    .trigger("click");
  expect(wrapper.find("form").exists()).toBe(true);
  expect(wrapper.find("input").element.value).toBe("Acme");
  expect(policy_invoke).toHaveBeenCalledTimes(1);
});

test("retiring enterprise ingress clears parameters/errors and ignores its late directory", async () => {
  const item = (
    record("old").action as Extract<
      BackendRequest["action"],
      { kind: "policy" }
    >
  ).directory;
  if (item.task.kind !== "script") throw Error("script");
  item.task.parameterSchema = {
    type: "object",
    properties: { name: { type: "string", minLength: 2 } },
    required: ["name"],
  };
  let resolveOld!: (value: unknown) => void;
  const directory = vi
    .fn()
    .mockResolvedValueOnce({ items: [item], nextCursor: null, asOf: 1 })
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOld = resolve;
        }),
    )
    .mockResolvedValue({ items: [], nextCursor: null, asOf: 2 });
  const wrapper = mount(PolicySelfService, {
    props: {
      port: { policy_directory: directory } as unknown as SelfServicePort,
      records: [],
    },
  });
  try {
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Policy")!
      .trigger("click");
    await wrapper.get("input").setValue("x");
    await wrapper.get("form").trigger("submit");
    expect(wrapper.find('[role="alert"]').exists()).toBe(true);
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "刷新任务")!
      .trigger("click");
    currentUser.value = {
      ...currentUser.value,
      generation: "retired-generation",
    } as NonNullable<typeof currentUser.value>;
    await flushPromises();
    resolveOld({ items: [item], nextCursor: null, asOf: 1 });
    await flushPromises();
    expect(wrapper.find("form").exists()).toBe(false);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain("请修正");
    expect(wrapper.text()).not.toContain("Read selected state");
  } finally {
    wrapper.unmount();
    currentUser.value = undefined;
  }
});

test("Device history restores original AI consent independently of enterprise identity", async () => {
  const original = record("device-original");
  if (original.action.kind !== "policy") throw Error("policy");
  original.action.access = { kind: "device" };
  original.action.directory.subject = { kind: "device" };
  original.state = "awaitingConfirmation";
  original.trigger = {
    ...original.trigger,
    kind: "ai",
    context: { id: "retired", generation: "old" },
    config: { id: "ai", revision: "1" },
    conversation: "original-ai",
    toolCall: "original-tool",
  } as BackendRequest["trigger"];
  const policy_confirm = vi.fn(async () => ({
    ...original,
    state: "submitting",
  }));
  const policy_invoke = vi.fn();
  const wrapper = mount(PolicySelfService, {
    props: {
      port: {
        policy_directory: async () => ({
          items: [],
          nextCursor: null,
          asOf: 1,
        }),
        policy_confirm,
        policy_invoke,
      } as unknown as SelfServicePort,
      records: [original],
    },
  });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("设备准入 · 无需企业登录");
    expect(wrapper.text()).toContain("original-ai / original-tool");
    expect(wrapper.text()).not.toContain("principal");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "确认上述操作")!
      .trigger("click");
    await flushPromises();
    expect(policy_confirm).toHaveBeenCalledExactlyOnceWith({
      requestId: "device-original",
    });
    expect(policy_invoke).not.toHaveBeenCalled();
  } finally {
    wrapper.unmount();
  }
});

test("MDM settings expose only an authorized enrollment and do not auto invoke", async () => {
  const original = record("manual-enrollment");
  if (original.action.kind !== "policy") throw Error("fixture");
  const originalDirectory = original.action.directory;
  const item = structuredClone(originalDirectory);
  item.task = {
    kind: "enrollment",
    organization: "tenant",
    entry: { kind: "macos", url: "https://mdm.example/enroll" },
  };
  const policy_invoke = vi.fn();
  const port = {
    policy_directory: async () => ({
      items: [originalDirectory, item],
      nextCursor: null,
      asOf: 1,
    }),
    policy_invoke,
  } as unknown as SelfServicePort;
  const wrapper = mount(PolicySelfService, {
    props: { port, records: [], kind: "enrollment", allowNewRequests: true },
  });
  await flushPromises();
  expect(wrapper.findAll(".resource-card")).toHaveLength(1);
  expect(wrapper.text()).toContain("注册 MDM");
  expect(policy_invoke).not.toHaveBeenCalled();
  await wrapper.find(".resource-card button").trigger("click");
  await wrapper.find("form").trigger("submit");
  expect(wrapper.find('[aria-label="人工执行确认"]').exists()).toBe(true);
  await wrapper.setProps({ allowNewRequests: false, allowConfirm: false });
  expect(wrapper.find('[aria-label="人工执行确认"]').exists()).toBe(false);
  expect(policy_invoke).not.toHaveBeenCalled();
  expect(wrapper.findAll(".resource-card")).toHaveLength(0);
  wrapper.unmount();
  const closed = mount(PolicySelfService, {
    props: {
      port: {
        ...port,
        policy_directory: async () => ({
          items: [],
          nextCursor: null,
          asOf: 1,
        }),
      },
      records: [],
      kind: "enrollment",
    },
  });
  await flushPromises();
  expect(closed.text()).toContain("当前没有组织授权的 MDM 注册入口");
  expect(closed.findAll("button").some((b) => b.text() === "注册 MDM")).toBe(
    false,
  );
  closed.unmount();
});

test.each([undefined, 12, "   "])(
  "unnamed policies remain distinguishable in selection and confirmation: %s",
  async (displayName) => {
    const original = record("unnamed-request");
    if (original.action.kind !== "policy") throw Error("fixture");
    const item = original.action.directory;
    item.selfService.presentation =
      displayName === undefined ? {} : { displayName };
    const second = structuredClone(item);
    second.policyId = "other-policy";
    const port = {
      policy_directory: async () => ({
        items: [item, second],
        nextCursor: null,
        asOf: 1,
      }),
    } as unknown as SelfServicePort;
    const human = mount(PolicySelfService, { props: { port, records: [] } });
    const preparation = mount(PolicyPreparation, {
      props: { record: original },
    });
    try {
      await flushPromises();
      expect(human.findAll("button").map((b) => b.text())).toContain(
        "策略 other-policy",
      );
      await human
        .findAll("button")
        .find((b) => b.text() === "策略 policy")!
        .trigger("click");
      await human.get("textarea").setValue(JSON.stringify("Acme"));
      await human.get("form").trigger("submit");
      expect(human.get('[aria-label="人工执行确认"]').text()).toContain(
        "策略 policy",
      );
      expect(preparation.get("h3").text()).toBe("策略 policy");
      expect(preparation.text()).toContain("未提供");
    } finally {
      human.unmount();
      preparation.unmount();
    }
  },
);


test("discovery continues beyond sixteen pages and invokes using the actual page", async () => {
  let page = 0;
  const item = (record("fixture").action as Extract<BackendRequest["action"], {kind:"policy"}>).directory;
  item.task = { kind: "software", resource: {id:"resource",version:"1",variant:"macos"},runAs:"system",target:{kind:"device"} };
  const policy_directory = vi.fn(async ({after}: {after:string|null}) => {
    expect(after).toBe(page === 0 ? null : `cursor-${page}`);
    ++page;
    return {items:page === 18 ? [item] : [],nextCursor:page < 18 ? `cursor-${page}` : null,asOf:1};
  });
  const policy_invoke = vi.fn(async () => record("original"));
  const w = mount(PolicySelfService, {props:{port:{policy_directory,policy_invoke} as unknown as SelfServicePort,records:[]}});
  try {
    await flushPromises();
    expect(w.text()).toContain("目录尚未完整");
    for (let n=1;n<18;n++) { await w.findAll("button").find(b=>b.text()==="继续加载")!.trigger("click"); await flushPromises(); }
    expect(w.text()).toContain("目录已读取完毕");
    await w.findAll("button").find(b=>b.text()==="Policy")!.trigger("click");
    await w.get("form").trigger("submit");
    await w.findAll("button").find(b=>b.text()==="确认并执行")!.trigger("click");
    await flushPromises();
    expect(policy_invoke).toHaveBeenCalledWith(expect.objectContaining({locator:{after:"cursor-17"}}));
    expect(policy_directory).toHaveBeenCalledTimes(18);
  } finally { w.unmount(); }
});

test("a repeated cursor does not falsely complete the catalog or discard loaded items", async () => {
  const item = (record("fixture").action as Extract<BackendRequest["action"], {kind:"policy"}>).directory;
  const policy_directory = vi.fn().mockResolvedValueOnce({items:[item],nextCursor:"same",asOf:1})
    .mockResolvedValue({items:[],nextCursor:"same",asOf:1});
  const w = mount(PolicySelfService,{props:{port:{policy_directory} as unknown as SelfServicePort,records:[]}});
  try {
    await flushPromises();
    await w.findAll("button").find(b=>b.text()==="继续加载")!.trigger("click"); await flushPromises();
    expect(w.text()).toContain("目录读取未完成");
    expect(w.text()).toContain("当前加载 1 条");
    expect(w.text()).not.toContain("目录已读取完毕");
  } finally {w.unmount();}
});
