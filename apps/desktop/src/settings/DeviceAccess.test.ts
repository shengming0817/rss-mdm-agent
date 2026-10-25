import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import DeviceAccess from "./DeviceAccess.vue";
import type { DeviceAccessPort } from "../self-service/native";
const task = { task: "task-id", binding: "binding-id", createName: "newuser" };
const directory = {
  kind: "directory" as const,
  policies: [],
  nextCursor: null,
  bindings: [],
};
type Reply = Awaited<ReturnType<DeviceAccessPort>>;
function portWithSetup(setup: () => Promise<Reply>) {
  return vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "ready",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "unsupported",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
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
it("Mac setup uses native password preparation and keeps backend and system facts separate", async () => {
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "ready",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "unsupported",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
    if (input.kind === "directory") return directory;
    if (input.kind === "tasks") return { kind: "tasks", tasks: [task] };
    if (input.kind === "macStatus")
      return {
        kind: "macStatus",
        configuration: {
          authentication: "password",
          passwordProvider: "00000000-0000-0000-0000-000000000001",
          login: "require",
          unlock: "require",
          filevault: "require",
          offlineGraceSeconds: null,
          registrationGraceSeconds: null,
          maintenanceAccounts: ["maintenance"],
        },
        passwordExchange: {
          attemptId: "00000000-0000-0000-0000-000000000002",
          state: "succeeded",
        },
        extensionInstalled: true,
        deviceRegistered: true,
        userRegistered: true,
        loginReady: true,
        nativeDeviceRegistered: null,
        nativeUserRegistered: null,
        observedAt: null,
      };
    if (input.kind === "prepareMac") return { kind: "prepared" };
    throw new Error("Windows factor flow must not run on Mac");
  });
  const wrapper = mount(DeviceAccess, { props: { port, platform: "macos" } });
  await flushPromises();
  expect(wrapper.text()).toContain("尚需实际登录确认");
  expect(wrapper.text()).toContain("暂无系统回执");
  expect(wrapper.text()).toContain("服务端已验证");
  expect(wrapper.text()).toContain("同步仍须分别核实");
  expect(wrapper.text()).toContain("离线可用性及恢复入口需按实际配置核实");
  expect(wrapper.text()).not.toContain("本地密码登录始终保留");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "设置本机账号密码")!
    .trigger("click");
  await wrapper.find('input[type="password"]').setValue("transient-password");
  await wrapper.find("form").trigger("submit");
  await flushPromises();
  expect(port.mock.calls.some(([input]) => input.kind === "prepareMac")).toBe(
    true,
  );
  expect(
    port.mock.calls.some(
      ([input]) => input.kind === "prepare" || input.kind === "confirmFactor",
    ),
  ).toBe(false);
  expect(wrapper.find("form").exists()).toBe(false);
  expect(wrapper.html()).not.toContain("transient-password");
  wrapper.unmount();
});

it("passes the opaque directory cursor unchanged", async () => {
  const cursor = "signed/a+b==";
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "tasks") return { kind: "tasks", tasks: [] };
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "ready",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "unsupported",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
    if (input.kind === "directory")
      return { ...directory, nextCursor: input.cursor ? null : cursor };
    throw new Error("unexpected operation");
  });
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "更多可用策略")!
    .trigger("click");
  await flushPromises();
  expect(port).toHaveBeenCalledWith({ kind: "directory", cursor });
  expect(wrapper.text()).not.toContain("更多可用策略");
  wrapper.unmount();
});

it("shows native account provenance and the actual unbind disposition", async () => {
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "tasks") return { kind: "tasks", tasks: [] };
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "ready",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "unsupported",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
    if (input.kind === "directory")
      return {
        ...directory,
        bindings: [
          {
            id: "binding",
            bindingGeneration: 2,
            factorGeneration: 1,
            revision: 3,
            accountDisposition: "disable",
            appliedRevision: 3,
            receivedRevision: 3,
            revoked: true,
            account: {
              platform: "windows",
              sid: "S-1-12-1-1-2-3-4",
              authority: {
                kind: "entra",
                tenantId: "tenant",
                objectId: "object",
              },
            },
            configuration: {
              mac: null,
              windows: { kind: "rss_factor", accounts: ["entra"] },
              selfEnrollment: true,
              enabled: false,
              role: null,
            },
            effect: { state: "unbound", disposition: "disable" },
          },
        ],
      };
    throw new Error("unexpected operation");
  });
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  expect(wrapper.text()).toContain("Entra joined 账号");
  expect(wrapper.text()).toContain("本机登录已限制");
  expect(wrapper.text()).not.toContain("系统账号保持原状");
  wrapper.unmount();
});

it.each([
  ["rebootRequired", "请重启电脑后重试应用"],
  ["maintenanceDisabled", "组件已由管理员停用"],
] as const)(
  "shows %s independently of historical applied receipts",
  async (component, text) => {
    const port = vi.fn<DeviceAccessPort>(async (input) => {
      if (input.kind === "windowsStatus")
        return {
          kind: "windowsStatus",
          component,
          factorActive: true,
          signInRequired: false,
          hello: {
            availability: "unsupported",
            desiredEnabled: null,
            retired: false,
            replacementRequired: false,
            facts: null,
          },
        };
      if (input.kind === "tasks") return { kind: "tasks", tasks: [] };
      if (input.kind === "directory")
        return {
          ...directory,
          bindings: [
            {
              id: "binding",
              bindingGeneration: 1,
              factorGeneration: 1,
              revision: 1,
              accountDisposition: "keep",
              appliedRevision: 1,
              receivedRevision: 1,
              revoked: false,
              account: {
                platform: "windows",
                sid: "S-1-5-21-1-2-3-1001",
                authority: { kind: "local" },
              },
              configuration: {
                mac: null,
                windows: { kind: "rss_factor", accounts: ["local"] },
                selfEnrollment: true,
                enabled: true,
                role: null,
              },
              effect: {
                state: "applied",
                account: {
                  platform: "windows",
                  sid: "S-1-5-21-1-2-3-1001",
                  authority: { kind: "local" },
                },
                role: "standard",
                mac: null,
                windows: null,
              },
            },
          ],
        };
      throw new Error("unexpected operation");
    });
    const wrapper = mount(DeviceAccess, { props: { port } });
    await flushPromises();
    expect(wrapper.text()).toContain(text);
    expect(wrapper.text()).toContain("服务端最近回执：RSS 登录已应用");
    wrapper.unmount();
  },
);
it("keeps local diagnosis when the server is unavailable", async () => {
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "rebootRequired",
        factorActive: false,
        signInRequired: true,
        hello: {
          availability: "unsupported",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
    throw new Error("offline");
  });
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  expect(wrapper.text()).toContain("请重启电脑后重试应用");
  expect(wrapper.text()).toContain("请注销 Windows 并重新登录");
  expect(wrapper.text()).toContain("操作尚未确认");
  wrapper.unmount();
});

it("keeps Hello hardware proof pending independently of RSS CP installation", async () => {
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "notInstalled",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "eligible",
          desiredEnabled: true,
          retired: false,
          replacementRequired: false,
          facts: {
            factorGeneration: 1,
            tenantId: "tenant",
            objectId: "object",
            policy: "present",
            registration: "present",
            hardwareKey: "unknown",
            keyId: "key",
            observedAt: 100,
          },
        },
      };
    if (input.kind === "directory") return directory;
    if (input.kind === "tasks") return { kind: "tasks", tasks: [] };
    if (input.kind === "openWindowsHello")
      return { kind: "windowsHelloSettingsRequested" };
    throw new Error("unexpected operation");
  });
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  expect(wrapper.text()).toContain("Windows Hello 企业登录");
  expect(wrapper.text()).toContain("用户密钥硬件保护：待核验");
  expect(wrapper.text()).not.toContain("组件未安装，请联系管理员");
  expect(wrapper.text()).not.toContain("设置验证器");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "打开 Windows 登录选项")!
    .trigger("click");
  await flushPromises();
  expect(port).toHaveBeenCalledWith({ kind: "openWindowsHello" });
  expect(wrapper.text()).toContain("打开设置不代表注册成功");
  wrapper.unmount();
});

it("shows completed Hello retirement without asking for cleanup again", async () => {
  const facts = {
    factorGeneration: 1,
    tenantId: "tenant",
    objectId: "object",
    policy: "absent" as const,
    registration: "absent" as const,
    hardwareKey: "unknown" as const,
    keyId: null,
    observedAt: 100,
  };
  const port = vi.fn<DeviceAccessPort>(async (input) => {
    if (input.kind === "windowsStatus")
      return {
        kind: "windowsStatus",
        component: "notInstalled",
        factorActive: false,
        signInRequired: false,
        hello: {
          availability: "eligible",
          desiredEnabled: false,
          retired: true,
          replacementRequired: false,
          facts,
        },
      };
    if (input.kind === "tasks") return { kind: "tasks", tasks: [] };
    if (input.kind === "directory")
      return {
        ...directory,
        bindings: [
          {
            id: "binding",
            bindingGeneration: 1,
            factorGeneration: 1,
            revision: 2,
            accountDisposition: "keep",
            appliedRevision: 2,
            receivedRevision: 2,
            revoked: true,
            account: {
              platform: "windows",
              sid: "S-1-12-1-1-2-3-4",
              authority: {
                kind: "entra",
                tenantId: "tenant",
                objectId: "object",
              },
            },
            configuration: {
              windows: { kind: "whfb" },
              mac: null,
              enabled: false,
              selfEnrollment: false,
              role: null,
            },
            effect: {
              state: "whfb_retired",
              account: {
                platform: "windows",
                sid: "S-1-12-1-1-2-3-4",
                authority: {
                  kind: "entra",
                  tenantId: "tenant",
                  objectId: "object",
                },
              },
              disposition: "keep",
              facts,
            },
          },
        ],
      };
    throw new Error("unexpected operation");
  });
  const wrapper = mount(DeviceAccess, { props: { port } });
  await flushPromises();
  expect(wrapper.text()).toContain("WHfB 原生策略与用户凭据清理已确认");
  expect(wrapper.text()).not.toContain("请按管理员指引完成原生清理");
  expect(wrapper.text()).not.toContain("已请求移除，等待本机确认");
  wrapper.unmount();
});
