import { flushPromises, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import MdmRegistration from "./MdmRegistration.vue";
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
    const wrapper = shallowMount(MdmRegistration);
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
  const wrapper = shallowMount(MdmRegistration);
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
