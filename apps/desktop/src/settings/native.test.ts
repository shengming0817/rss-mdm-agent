import { beforeEach, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import App from "../App.vue";
import { currentUser } from "../test-users";
import { nativeHost } from "./native";

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
        return { schemaVersion: 1, kind: "accountStatus", current };
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
      return { schemaVersion: 1, kind: "accountStatus", current: next };
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
