import { beforeEach, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import App from "../App.vue";
import { currentUser } from "../test-users";
import { nativeHost, type ServicePort } from "./native";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
const context = (mode: "device" | "test" | "user_device"): UserContext => ({
  schemaVersion: 1,
  kind: "userContext",
  generation: "old-generation",
  user: {
    schemaVersion: 1,
    kind: "testUser",
    userId: "actor",
    displayName: "Alice",
    nameKey: "alice",
  },
  identity:
    mode === "user_device"
      ? {
          mode,
          tenantId: "tenant",
          principalId: "actor",
          authorityId: "enterprise",
          organizationId: "org",
          expiresAtMs: 1000,
        }
      : {
          mode: "device",
          tenantId: "local",
          principalId: "actor",
          authorityId: mode === "test" ? "desktop-test" : "desktop-device",
        },
});
const status = {
  schemaVersion: 1,
  kind: "hostStatus",
  generation: 2,
  phase: "ready",
  source: "bundled_resource",
  version: "test",
  recent: [],
};
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  currentUser.value = undefined;
});

it("keeps the registration view mounted while the native owner retires and replaces the account", async () => {
  vi.useFakeTimers();
  let current: UserContext | undefined = context("device");
  let finish!: (value: unknown) => void;
  let finishRead!: (value: unknown) => void;
  let holdRead = false;
  let registering = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 1, kind: "testUserPage", users: [], current };
    if (command === "account_organizations")
      return { schemaVersion: 1, kind: "accountSettings", organizations: [] };
    if (command === "account_status" && holdRead) {
      holdRead = false;
      return new Promise((resolve) => {
        finishRead = resolve;
      });
    }
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        ...(!registering && current ? { current } : {}),
      };
    if (command === "agent_request_registration") {
      registering = true;
      return new Promise((resolve) => {
        finish = resolve;
      });
    }
    if (command === "ai_host_status") return status;
    throw Error("unexpected command");
  });
  const wrapper = mount(App, {
    global: {
      stubs: {
        Workspace: {
          name: "Workspace",
          props: ["servicePort", "ready"],
          template: "<div />",
        },
      },
    },
  });
  try {
    await flushPromises();
    const workspace = wrapper.findComponent({ name: "Workspace" });
    const owner = workspace.vm;
    holdRead = true;
    await vi.advanceTimersByTimeAsync(2000);
    const pending = (
      workspace.props("servicePort") as ServicePort
    ).requestRegistration();
    finishRead({
      schemaVersion: 1,
      kind: "accountStatus",
      binding: "unbound",
    });
    await flushPromises();
    const reads = vi
      .mocked(invoke)
      .mock.calls.filter(([c]) => c === "account_status").length;
    await vi.advanceTimersByTimeAsync(6000);
    expect(wrapper.findComponent({ name: "Workspace" }).vm).toBe(owner);
    expect(workspace.props("ready")).toBe(false);
    expect(
      vi.mocked(invoke).mock.calls.filter(([c]) => c === "account_status"),
    ).toHaveLength(reads);
    expect(wrapper.findComponent({ name: "LoginScreen" }).exists()).toBe(false);
    current = {
      ...context("user_device"),
      generation: "registered-generation",
    };
    registering = false;
    finish({ phase: "active", registered: true, credential: "active" });
    await pending;
    await flushPromises();
    expect(currentUser.value?.generation).toBe("registered-generation");
    expect(wrapper.findComponent({ name: "Workspace" }).vm).not.toBe(owner);
    expect(wrapper.findComponent({ name: "Workspace" }).props("ready")).toBe(
      true,
    );
  } finally {
    wrapper.unmount();
    vi.useRealTimers();
  }
});

it("retains the registration authentication error after the old account is retired", async () => {
  let current: UserContext | undefined = context("device");
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 1, kind: "testUserPage", users: [], current };
    if (command === "account_organizations")
      return { schemaVersion: 1, kind: "accountSettings", organizations: [] };
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        ...(current ? { current } : {}),
      };
    if (command === "agent_request_registration") {
      current = undefined;
      throw { code: "account/login/denied" };
    }
    if (command === "ai_host_status") return status;
    throw Error("unexpected command");
  });
  const wrapper = mount(App, {
    global: {
      stubs: {
        Workspace: {
          name: "Workspace",
          props: ["servicePort"],
          template: "<div />",
        },
      },
    },
  });
  try {
    await flushPromises();
    const port = wrapper
      .findComponent({ name: "Workspace" })
      .props("servicePort") as ServicePort;
    await expect(port.requestRegistration()).rejects.toMatchObject({
      code: "account/login/denied",
    });
    await flushPromises();
    expect(wrapper.text()).toContain("登录：账号或权限已失效");
    expect(wrapper.findComponent({ name: "Workspace" }).exists()).toBe(false);
    expect(currentUser.value).toBeUndefined();
  } finally {
    wrapper.unmount();
  }
});

it.each(["device", "test", "user_device"] as const)(
  "restart synchronizes %s identity and remounts the workspace",
  async (mode) => {
    let current = context(mode);
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "test_users")
        return { schemaVersion: 1, kind: "testUserPage", users: [], current };
      if (command === "account_organizations")
        return { schemaVersion: 1, kind: "accountSettings", organizations: [] };
      if (command === "account_status")
        return {
          schemaVersion: 1,
          kind: "accountStatus",
          binding: "unbound",
          current,
        };
      if (command === "ai_host_status") return status;
      if (command === "ai_restart_host") {
        current = { ...current, generation: "new-generation" };
        return status;
      }
      throw Error("unexpected command");
    });
    const wrapper = mount(App, {
      global: {
        stubs: { Workspace: { name: "Workspace", template: "<div />" } },
      },
    });
    try {
      await flushPromises();
      const workspace = wrapper.findComponent({ name: "Workspace" }).vm;
      await nativeHost()!.restart(1);
      await flushPromises();
      expect(currentUser.value?.generation).toBe("new-generation");
      expect(wrapper.findComponent({ name: "Workspace" }).vm).not.toBe(
        workspace,
      );
    } finally {
      wrapper.unmount();
    }
  },
);

it("synchronizes a rotated identity even when Host startup fails", async () => {
  currentUser.value = context("device");
  const next = { ...currentUser.value, generation: "rotated-generation" };
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "ai_restart_host") throw Error("startup failed");
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        current: next,
      };
    throw Error("unexpected command");
  });
  await expect(nativeHost()!.restart(1)).rejects.toThrow("startup failed");
  expect(currentUser.value).toEqual(next);
});

it("restart completion cannot replace a concurrently selected identity", async () => {
  currentUser.value = context("device");
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = nativeHost()!.restart(1);
  const selected = { ...context("test"), generation: "selected-generation" };
  currentUser.value = selected;
  finish(status);
  await pending;
  expect(currentUser.value).toEqual(selected);
  expect(invoke).toHaveBeenCalledTimes(1);
});
