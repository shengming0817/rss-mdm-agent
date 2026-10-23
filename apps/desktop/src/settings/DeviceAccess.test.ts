import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import DeviceAccess from "./DeviceAccess.vue";
import type { DeviceAccessPort } from "../self-service/native";
const task = { task: "task-id", binding: "binding-id", createName: "newuser" };
const directory = {
  kind: "directory" as const,
  policies: [],
  after: null,
  bindings: [],
};
type Reply = Awaited<ReturnType<DeviceAccessPort>>;
function portWithSetup(setup: () => Promise<Reply>) {
  return vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "directory") return directory;
    if (input.kind === "tasks") return { kind: "tasks", tasks: [task] };
    if (input.kind === "prepare") return setup();
    if (input.kind === "confirmFactor") return { kind: "prepared" };
    throw new Error("unexpected operation");
  });
}
it("does not present preparation acknowledgement as applied", async () => {
  const port = portWithSetup(async () => ({
    kind: "setup",
    task: task.task,
    secret: "TESTSECRET",
  }));
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "设置验证器")!
    .trigger("click");
  await flushPromises();
  const inputs = wrapper.findAll("input");
  await inputs[1].setValue("123456");
  await inputs[2].setValue("local-password");
  await wrapper.find("form").trigger("submit");
  await flushPromises();
  expect(wrapper.text()).toContain("正在等待 Agent 应用");
  expect(wrapper.text()).not.toContain("RSS 登录已应用");
  expect(wrapper.find("form").exists()).toBe(false);
  expect(wrapper.html()).not.toContain("TESTSECRET");
  wrapper.unmount();
});
it("discards a late setup response after the page is hidden", async () => {
  let resolve!: (value: Reply) => void;
  const port = portWithSetup(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "设置验证器")!
    .trigger("click");
  await wrapper.setProps({ active: false });
  resolve({ kind: "setup", task: task.task, secret: "LATESECRET" });
  await flushPromises();
  expect(wrapper.find("form").exists()).toBe(false);
  expect(wrapper.html()).not.toContain("LATESECRET");
  wrapper.unmount();
});
it("clears shown material when its native port owner changes", async () => {
  const port = portWithSetup(async () => ({
    kind: "setup",
    task: task.task,
    secret: "OLDSECRET",
  }));
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "设置验证器")!
    .trigger("click");
  await flushPromises();
  expect(wrapper.find("form").exists()).toBe(true);
  await wrapper.setProps({
    port: portWithSetup(async () => {
      throw new Error("no setup");
    }),
  });
  await flushPromises();
  expect(wrapper.find("form").exists()).toBe(false);
  expect(wrapper.html()).not.toContain("OLDSECRET");
  wrapper.unmount();
});
