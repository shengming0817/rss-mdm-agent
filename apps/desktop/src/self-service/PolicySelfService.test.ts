import { mount, flushPromises } from "@vue/test-utils";
import { expect, test, vi } from "vitest";
import PolicySelfService from "./PolicySelfService.vue";
import type { BackendRequest, SelfServicePort } from "./types";
const record = (id: string): BackendRequest => ({
  action: {
    kind: "policy",
    choice: {
      request: id,
      policyId: "policy",
      policyRevision: 1,
      policyVersion: "version",
      parameters: { name: "Acme" },
    },
    directory: {
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
        displayName: "Policy",
        description: "Read selected state",
        category: "Tools",
        keywords: [],
        prerequisites: "Current login",
        sideEffects: "Read only",
      },
    },
    user: {
      tenantId: "tenant",
      instanceId: "instance",
      principalId: "principal",
    },
    context: { id: "binding", generation: "generation" },
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
  expect(policy_invoke.mock.calls[0][0].parameters).toEqual({
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
