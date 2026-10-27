import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import MdmRegistration from "./MdmRegistration.vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import { taskCenterKey, createTaskCenter } from "../task-center/controller";
import {
  policyFixturePort,
  taskPort,
  taskPage,
  taskDetail,
} from "../../../../tests/self-service/support";
const native = vi.hoisted(() => ({ read: vi.fn() }));
vi.mock("./native", () => ({ readMdmStatus: native.read }));
function setup() {
  const offers = vi.fn(async () => []);
  const business = {
    ...policyFixturePort,
    offers,
    execute: vi.fn(),
    confirm: vi.fn(),
  };
  const tasks = createTaskCenter(taskPort(), business);
  return { offers, business, tasks };
}
it("permits enrollment only for unenrolled devices without active original records", async () => {
  for (const state of [
    "unenrolled",
    "this_organization",
    "other_organization",
    "unknown",
  ] as const) {
    const f = setup();
    native.read.mockResolvedValue({ state });
    const wrapper = shallowMount(MdmRegistration, {
      props: { executionPort: f.business },
      global: { provide: { [taskCenterKey as symbol]: f.tasks } },
    });
    await flushPromises();
    expect(
      wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
    ).toBe(state === "unenrolled");
    expect(wrapper.getComponent(PolicySelfService).props("allowConfirm")).toBe(
      state === "unenrolled",
    );
    wrapper.unmount();
    f.tasks.dispose();
  }
});
it("retains active enrollment identity and presents its original task", async () => {
  const f = setup(),
    detail = taskDetail("enrollment-original");
  if (detail.facts.kind !== "execution") throw Error("fixture");
  detail.facts.value.action.operation.action = "backend-enrollment";
  const read = vi
    .spyOn(f.tasks.tasks!, "list")
    .mockResolvedValue(taskPage([detail]));
  native.read.mockResolvedValue({ state: "unenrolled" });
  const wrapper = shallowMount(MdmRegistration, {
    props: { executionPort: f.business },
    global: { provide: { [taskCenterKey as symbol]: f.tasks } },
  });
  await flushPromises();
  expect(
    wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
  ).toBe(false);
  expect(wrapper.text()).toContain("查看原注册任务");
  expect(read).toHaveBeenCalled();
  wrapper.unmount();
  f.tasks.dispose();
});
it("a failed service read never assumes an unenrolled device", async () => {
  const f = setup();
  native.read.mockRejectedValue(Error("offline"));
  const wrapper = shallowMount(MdmRegistration, {
    props: { executionPort: f.business },
    global: { provide: { [taskCenterKey as symbol]: f.tasks } },
  });
  await flushPromises();
  expect(wrapper.text()).toContain("无法读取本机服务");
  expect(wrapper.findComponent(PolicySelfService).exists()).toBe(false);
  wrapper.unmount();
  f.tasks.dispose();
});
