import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import Tasks from "./Tasks.vue";
import PolicyPreparation from "./PolicyPreparation.vue";
import { createController } from "./controller";
import {
  policyRecord,
  policyFixturePort,
  snapshot,
} from "../../../../tests/self-service/support";
it("queries, confirms and cancels the original policy through the existing request view", async () => {
  const record = policyRecord(),
    request = "original-policy";
  const port = {
    ...policyFixturePort,
    snapshot: vi.fn(async () => ({ ...snapshot(), preparations: [record] })),
    execute: vi.fn(),
    confirm: vi.fn(),
    cancel: vi.fn(),
    policy_confirm: vi.fn(async () => record),
    policy_status: vi.fn(async () => ({
      kind: "preparation" as const,
      value: record,
    })),
    policy_invoke: vi.fn(),
  };
  const c = createController(port);
  await c.refresh();
  const w = mount(Tasks, { props: { controller: c } });
  try {
    const prep = () => w.getComponent(PolicyPreparation);
    await prep()
      .findAll("button")
      .find((b) => b.text() === "确认上述操作")!
      .trigger("click");
    await flushPromises();
    expect(port.policy_confirm).toHaveBeenCalledWith({ requestId: request });
    await prep()
      .findAll("button")
      .find((b) => b.text() === "查询原请求与结果")!
      .trigger("click");
    await flushPromises();
    expect(port.policy_status).toHaveBeenCalledWith({ requestId: request });
    await prep()
      .findAll("button")
      .find((b) => b.text() === "撤销原请求")!
      .trigger("click");
    await flushPromises();
    expect(port.cancel).toHaveBeenCalledWith({ requestId: request });
    expect(port.policy_invoke).not.toHaveBeenCalled();
  } finally {
    w.unmount();
    c.dispose();
  }
});
