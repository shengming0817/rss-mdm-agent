import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";

import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";

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
  currentUser.current = undefined;
});

it("synchronizes a rotated identity even when Host startup fails", async () => {
  currentUser.current = context("device");
  const next = { ...currentUser.current, generation: "rotated-generation" };
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
  expect(currentUser.current).toEqual(next);
});

it("restart completion cannot replace a concurrently selected identity", async () => {
  currentUser.current = context("device");
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = nativeHost()!.restart(1);
  const selected = { ...context("test"), generation: "selected-generation" };
  currentUser.current = selected;
  finish(status);
  await pending;
  expect(currentUser.current).toEqual(selected);
  expect(invoke).toHaveBeenCalledTimes(1);
});
