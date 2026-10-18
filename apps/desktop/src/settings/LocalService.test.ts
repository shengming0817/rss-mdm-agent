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
    importRegistration: async () => null,
    cancelRegistration: async () => {},
  };
  const wrapper = mount(LocalService, { props: { port, fixture: true } });
  await flushPromises();
  const button = (text: string) =>
    wrapper.findAll("button").find((b) => b.text() === text)!;
  await button("登录并注册此设备").trigger("click");
  await flushPromises();
  expect(request.mock.calls).toEqual([[undefined, undefined]]);
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
