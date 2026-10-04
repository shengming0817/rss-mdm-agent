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
      resource: { id: "resource", version: "1", variant: "macos" },
      parameterSchema: { type: "object" },
      parameterSources: { name: { kind: "input" } },
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
    actor: {
      tenantId: "tenant",
      instanceId: "instance",
      principalId: "principal",
    },
    generation: "generation",
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
        collectedAt: 1,
        quality: "complete",
        schemaValid: true,
        budgetValid: true,
        trusted: true,
        output: { message: "historical-result" },
        outputReference: null,
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
