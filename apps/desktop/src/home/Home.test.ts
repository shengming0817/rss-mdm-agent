import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import Home from "./Home.vue";
import { createController } from "../self-service/controller";
const controller = () =>
  createController(null, {
    available: [],
    preparations: [],
    requests: [],
    selected: null,
    next: null,
  });
it("shows device values as plain text and exposes all addresses without an unavailable banner", async () => {
  const c = controller();
  const wrapper = mount(Home, {
    props: {
      controller: c,
      preview: true,
      draft: "",
      connected: false,
      device: {
        computerName: "<img src=x>",
        ipAddresses: ["192.0.2.24", "2001:db8::24"],
        loginUser: "studio",
      },
    },
  });
  expect(wrapper.get(".identity-name").text()).toBe("<img src=x>");
  expect(wrapper.find(".identity-name img").exists()).toBe(false);
  expect(wrapper.text()).toContain("192.0.2.24");
  expect(wrapper.text()).toContain("+1");
  expect(wrapper.get('[title="192.0.2.24、2001:db8::24"]').exists()).toBe(true);
  expect(wrapper.text()).toContain("studio");
  expect(wrapper.text()).not.toContain("设备信息暂不可用");
  await wrapper.setProps({ device: null });
  expect(wrapper.get(".computer-identity").text()).not.toContain("studio");
  expect(wrapper.get(".computer-identity").text()).not.toContain("192.0.2.24");
  wrapper.unmount();
  c.dispose();
});
