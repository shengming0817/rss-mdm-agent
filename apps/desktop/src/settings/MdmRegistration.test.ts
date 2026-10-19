import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import MdmRegistration from "./MdmRegistration.vue";
import type { SelfServicePort } from "../self-service/types";
import PolicySelfService from "../self-service/PolicySelfService.vue";
const fixtures = vi.hoisted(() => ({
  read: vi.fn(),
  snapshot: vi.fn(),
  status: vi.fn(),
}));
vi.mock("./native", () => ({ readMdmStatus: fixtures.read }));
vi.mock("../self-service/native", () => ({
  nativePort: () => ({
    snapshot: fixtures.snapshot,
    policy_status: fixtures.status,
  }),
}));

const executionPort = {
  snapshot: fixtures.snapshot,
  policy_status: fixtures.status,
} as unknown as SelfServicePort;
it("only permits a new request when unenrolled without an unfinished original", async () => {
  for (const state of [
    "unenrolled",
    "this_organization",
    "other_organization",
    "unknown",
  ]) {
    fixtures.read.mockResolvedValue({ state, observedAt: 1 });
    fixtures.snapshot.mockResolvedValue({
      preparations: [],
      available: [],
      requests: [],
    });
    const wrapper = shallowMount(MdmRegistration, { props: { executionPort } });
    await flushPromises();
    expect(
      wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
    ).toBe(state === "unenrolled");
    expect(wrapper.getComponent(PolicySelfService).props("allowConfirm")).toBe(
      state === "unenrolled",
    );
    wrapper.unmount();
  }
  fixtures.read.mockResolvedValue({ state: "unenrolled", observedAt: 1 });
  fixtures.snapshot.mockResolvedValue({
    preparations: [
      {
        state: "ready",
        action: {
          kind: "policy",
          directory: { task: { kind: "enrollment" } },
          choice: { request: "original" },
        },
      },
    ],
    available: [],
    requests: [],
  });
  const wrapper = shallowMount(MdmRegistration, { props: { executionPort } });
  await flushPromises();
  expect(
    wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
  ).toBe(false);
  expect(wrapper.getComponent(PolicySelfService).props("allowConfirm")).toBe(
    true,
  );
  expect(wrapper.text()).toContain("已有注册流程");
  fixtures.read.mockRejectedValue(Error("service unavailable"));
  await wrapper.find("button").trigger("click");
  await flushPromises();
  expect(wrapper.findComponent(PolicySelfService).exists()).toBe(false);
  wrapper.unmount();
});

it("reads MDM facts without enabling tasks before Agent identity is ready and stops hidden polling", async () => {
  const read = vi
    .fn()
    .mockResolvedValue({ state: "unenrolled", observedAt: 1 });
  const snapshot = vi
    .fn()
    .mockResolvedValue({ preparations: [], available: [], requests: [] });
  const wrapper = shallowMount(MdmRegistration, {
    props: {
      fixture: true,
      active: false,
      agentReady: false,
      servicePort: { mdmStatus: read },
      executionPort: { snapshot } as unknown as SelfServicePort,
    },
  });
  try {
    await flushPromises();
    expect(read).not.toHaveBeenCalled();
    await wrapper.setProps({ active: true });
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(snapshot).not.toHaveBeenCalled();
    expect(wrapper.findComponent(PolicySelfService).exists()).toBe(false);
    await wrapper.setProps({ agentReady: true });
    await flushPromises();
    expect(snapshot).toHaveBeenCalledTimes(1);
    expect(
      wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
    ).toBe(true);
    read.mockResolvedValue({ state: "other_organization", observedAt: 2 });
    await wrapper.get("button").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("其他组织管理");
    expect(
      wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
    ).toBe(false);
    expect(wrapper.getComponent(PolicySelfService).props("allowConfirm")).toBe(
      false,
    );
  } finally {
    wrapper.unmount();
  }
});
