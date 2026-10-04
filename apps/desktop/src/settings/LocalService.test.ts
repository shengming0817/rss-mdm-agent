import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import LocalService from "./LocalService.vue";
import type { ServicePort } from "./native";
import type { RegistrationStatus } from "./service-contract";
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
    read: async () => ({
      phase: "connected",
      status: { protocol: 10, readiness: { phase: "registrationRequired" } },
    }),
    registration: async () => state,
    requestRegistration: request,
  };
  const wrapper = mount(LocalService, { props: { port, fixture: true } });
  await flushPromises();
  const button = (text: string) =>
    wrapper.findAll("button").find((b) => b.text() === text)!;
  await button("发起设备注册").trigger("click");
  await flushPromises();
  expect(request.mock.calls).toEqual([[]]);
  expect(wrapper.text()).toContain("public-grant");
  state = { phase: "rejected", registered: true, credential: "retired" };
  await button("重新查询").trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("设备证书已退休");
  expect(wrapper.text()).not.toContain("本机设备身份已激活");
  expect(request).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});
