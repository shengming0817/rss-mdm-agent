import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import MdmRegistration from "./MdmRegistration.vue";
import type { SelfServicePort } from "../self-service/types";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import TaskDetail from "../self-service/TaskDetail.vue";
import { snapshot as emptySnapshot } from "../../../../tests/self-service/support";
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

it("discards delayed task receipts after readiness changes and refreshes the current owner", async () => {
  let finish!: (value: ReturnType<typeof emptySnapshot>) => void;
  const oldPort = {
    snapshot: vi.fn(
      () =>
        new Promise<ReturnType<typeof emptySnapshot>>((resolve) => {
          finish = resolve;
        }),
    ),
  } as unknown as SelfServicePort;
  const nextPort = {
    snapshot: vi.fn().mockResolvedValue(emptySnapshot()),
  } as unknown as SelfServicePort;
  const read = vi.fn().mockResolvedValue({ state: "unenrolled" });
  const wrapper = shallowMount(MdmRegistration, {
    props: {
      fixture: true,
      servicePort: { mdmStatus: read },
      executionPort: oldPort,
    },
  });
  try {
    await flushPromises();
    await wrapper.setProps({ agentReady: false, executionPort: nextPort });
    const stale = emptySnapshot();
    stale.requests = [
      {
        action: {
          operation: { action: "backend-enrollment" },
          requestId: "old",
        },
        status: { phase: "executing" },
      },
    ] as unknown as typeof stale.requests;
    finish(stale);
    await flushPromises();
    expect(wrapper.findComponent(TaskDetail).exists()).toBe(false);
    expect(wrapper.findComponent(PolicySelfService).exists()).toBe(false);
    expect(nextPort.snapshot).not.toHaveBeenCalled();
    expect(read).toHaveBeenCalledTimes(2);
    await wrapper.setProps({ agentReady: true });
    await flushPromises();
    expect(nextPort.snapshot).toHaveBeenCalledTimes(1);
    expect(
      wrapper.getComponent(PolicySelfService).props("allowNewRequests"),
    ).toBe(true);
  } finally {
    wrapper.unmount();
  }
});

it("invalidates hidden work, resumes on activation and stops polling after unmount", async () => {
  vi.useFakeTimers();
  let finish!: (value: { state: "unenrolled" }) => void;
  const read = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValue({ state: "other_organization" });
  const snapshot = vi.fn().mockResolvedValue(emptySnapshot());
  const wrapper = shallowMount(MdmRegistration, {
    props: {
      fixture: true,
      servicePort: { mdmStatus: read },
      executionPort: { snapshot } as unknown as SelfServicePort,
    },
  });
  try {
    await wrapper.setProps({ active: false });
    finish({ state: "unenrolled" });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(10000);
    expect(read).toHaveBeenCalledTimes(1);
    expect(snapshot).not.toHaveBeenCalled();
    expect(wrapper.text()).not.toContain("本机尚未注册 MDM。");
    await wrapper.setProps({ active: true });
    await flushPromises();
    expect(wrapper.text()).toContain("其他组织管理");
    expect(read).toHaveBeenCalledTimes(2);
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(10000);
    expect(read).toHaveBeenCalledTimes(2);
  } finally {
    if (wrapper.exists()) wrapper.unmount();
    vi.useRealTimers();
  }
});

it("shares the workspace directory owner instead of creating a proxied transport directory", async () => {
  const read = vi.fn(async () => ({ items: [], nextCursor: null, asOf: 1 }));
  const owner = {
    ...policyFixturePort,
    policy_directory: read,
    snapshot: async () => ({
      available: [],
      preparations: [],
      requests: [],
      selected: null,
      next: null,
    }),
    execute: vi.fn(),
    confirm: vi.fn(),
    cancel: vi.fn(),
  } as SelfServicePort;
  const catalog = createDirectory(owner);
  await catalog.load();
  const w = mount(MdmRegistration, {
    props: {
      fixture: true,
      executionPort: owner,
      servicePort: {
        mdmStatus: async () => ({ state: "unenrolled" as const }),
      },
    },
    global: { provide: { [directoryKey as symbol]: catalog } },
  });
  try {
    await flushPromises();
    expect(w.getComponent(PolicySelfService).text()).toContain("MDM 注册");
    expect(read).toHaveBeenCalledTimes(1);
  } finally {
    w.unmount();
    catalog.dispose();
  }
});
import { createDirectory, directoryKey } from "../self-service/directory";
import { policyFixturePort } from "../../../../tests/self-service/support";
