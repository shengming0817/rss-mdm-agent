import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import LocalService from "./LocalService.vue";
import type { ServicePort } from "./native";
import serviceViews from "../../../../tests/assistant/service-fixtures.json";
import type { RegistrationStatus, ServiceView } from "./service-contract";
it("requests one fixed machine registration and refreshes its original public progress", async () => {
  let state: RegistrationStatus = {
    phase: "unregistered",
    registered: false,
    credential: "absent",
  };
  const request = vi.fn(async () => {
    state = {
      ...state,
      phase: "awaitingAuthorization",
      grantId: "public-grant",
      publicKeyFingerprint: "a".repeat(64),
    };
    return state;
  });
  const port: ServicePort = {
    read: async () =>
      structuredClone(serviceViews.registrationRequired) as ServiceView,
    registration: async () => state,
    requestRegistration: request,
    mdmStatus: async () => ({ state: "unknown" }),
    importRegistration: async () => null,
    cancelRegistration: async () => {},
  };
  const wrapper = mount(LocalService, { props: { port, fixture: true } });
  await flushPromises();
  const button = (text: string) =>
    wrapper.findAll("button").find((b) => b.text() === text)!;
  await button("登录并注册此设备").trigger("click");
  await flushPromises();
  expect(request.mock.calls).toEqual([[undefined]]);
  expect(wrapper.text()).toContain("public-grant");
  state = { phase: "rejected", registered: true, credential: "retired" };
  await button("重新查询").trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("设备证书已退休");
  expect(wrapper.text()).not.toContain("本机设备身份已激活");
  expect(request).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});

it("stops an explicit registration wait without starting another machine request", async () => {
  let stop: ((reason: unknown) => void) | undefined;
  const request = vi.fn(
    () =>
      new Promise<RegistrationStatus>((_resolve, reject) => {
        stop = reject;
      }),
  );
  const cancel = vi.fn(async () => {
    stop?.({
      code: "registration_cancelled",
      message: "已停止注册等待；已提交的原请求仍保留",
    });
  });
  const port: ServicePort = {
    read: async () =>
      structuredClone(serviceViews.registrationRequired) as ServiceView,
    registration: async () => ({
      phase: "unregistered",
      registered: false,
      credential: "absent",
    }),
    requestRegistration: request,
    mdmStatus: async () => ({ state: "unknown" }),
    importRegistration: async () => null,
    cancelRegistration: cancel,
  };
  const wrapper = mount(LocalService, { props: { port, fixture: true } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "登录并注册此设备")!
    .trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("网络恢复后将自动授权");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "停止注册等待")!
    .trigger("click");
  await flushPromises();
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(request).toHaveBeenCalledTimes(1);
  expect(wrapper.text()).toContain("已提交的原请求仍保留");
  wrapper.unmount();
});

it("removes stale registration facts after the local service becomes unavailable", async () => {
  const read = vi.fn().mockResolvedValue(structuredClone(serviceViews.ready));
  const request = vi.fn();
  const port: ServicePort = {
    read,
    registration: async () => ({
      phase: "active",
      registered: true,
      credential: "active",
      grantId: "previous-registration",
    }),
    mdmStatus: async () => ({ state: "unenrolled" }),
    requestRegistration: request,
    importRegistration: async () => null,
    cancelRegistration: async () => {},
  };
  const wrapper = mount(LocalService, { props: { fixture: true, port } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("previous-registration");
    expect(wrapper.text()).not.toContain("正在处理原注册请求");
    expect(request).not.toHaveBeenCalled();
    read.mockRejectedValueOnce(new Error("offline"));
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "重新查询")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("previous-registration");
    expect(wrapper.find(".registration-badge.ready").exists()).toBe(false);
    expect(request).not.toHaveBeenCalled();
  } finally {
    wrapper.unmount();
  }
});

it("automatically recovers registration facts after a temporary read failure without starting another request", async () => {
  vi.useFakeTimers();
  const read = vi
    .fn()
    .mockResolvedValue(structuredClone(serviceViews.registrationRequired));
  const request = vi.fn();
  const port: ServicePort = {
    read,
    registration: vi
      .fn()
      .mockResolvedValueOnce({
        phase: "awaitingAuthorization",
        registered: false,
        credential: "absent",
        grantId: "original",
      })
      .mockResolvedValue({
        phase: "active",
        registered: true,
        credential: "active",
        grantId: "original",
      }),
    requestRegistration: request,
    mdmStatus: async () => ({ state: "unknown" }),
    importRegistration: async () => null,
    cancelRegistration: async () => {},
  };
  const wrapper = mount(LocalService, { props: { fixture: true, port } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("original");
    read.mockRejectedValueOnce(new Error("temporary outage"));
    await vi.advanceTimersByTimeAsync(30000);
    expect(wrapper.text()).not.toContain("original");
    read.mockResolvedValue(structuredClone(serviceViews.ready));
    await vi.advanceTimersByTimeAsync(30000);
    expect(wrapper.text()).toContain("original");
    expect(wrapper.find(".registration-badge.ready").exists()).toBe(true);
    read.mockResolvedValue({ phase: "unavailable" });
    await vi.advanceTimersByTimeAsync(30000);
    expect(wrapper.find(".registration-badge.ready").exists()).toBe(false);
    expect(request).not.toHaveBeenCalled();
    await wrapper.setProps({ active: false });
    const calls = read.mock.calls.length;
    await vi.advanceTimersByTimeAsync(10000);
    expect(read).toHaveBeenCalledTimes(calls);
  } finally {
    wrapper.unmount();
    vi.useRealTimers();
  }
});

it("keeps registration facts visible during background refresh and does not call generic unreadiness a persistence failure", async () => {
  vi.useFakeTimers();
  let finish: ((view: ServiceView) => void) | undefined;
  const read = vi
    .fn()
    .mockResolvedValueOnce(structuredClone(serviceViews.ready))
    .mockImplementation(
      () =>
        new Promise<ServiceView>((resolve) => {
          finish = resolve;
        }),
    );
  const request = vi.fn();
  const wrapper = mount(LocalService, {
    props: {
      fixture: true,
      port: {
        read,
        registration: async () => ({
          phase: "active",
          registered: true,
          credential: "active",
          grantId: "existing-identity",
        }),
        requestRegistration: request,
        mdmStatus: async () => ({ state: "unknown" }),
        importRegistration: async () => null,
        cancelRegistration: async () => {},
      },
    },
  });
  try {
    await flushPromises();
    await vi.advanceTimersByTimeAsync(30000);
    expect(read).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("existing-identity");
    expect(wrapper.text()).not.toContain("正在验证服务连接");
    const notReady = structuredClone(serviceViews.ready) as ServiceView;
    if (notReady.phase !== "connected")
      throw Error("connected fixture required");
    notReady.status.readiness.phase = "notReady";
    finish!(notReady);
    await flushPromises();
    expect(wrapper.text()).toContain("执行服务暂未就绪");
    expect(wrapper.text()).not.toContain("持久");
    expect(wrapper.text()).toContain("existing-identity");
    expect(request).not.toHaveBeenCalled();
  } finally {
    wrapper.unmount();
    vi.useRealTimers();
  }
});
