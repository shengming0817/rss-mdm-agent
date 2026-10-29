import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";

import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";

import { accountState, retireAccountOperations } from "../test-users";

import { nativeHost, nativeService, type ServicePort } from "./native";

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
  accountState.user = undefined;
});

it("synchronizes a rotated identity even when Host startup fails", async () => {
  accountState.user = context("device");
  const next = { ...accountState.user, generation: "rotated-generation" };
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
  expect(accountState.user).toEqual(next);
});

it("restart completion cannot replace a concurrently selected identity", async () => {
  accountState.user = context("device");
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = nativeHost()!.restart(1);
  const selected = { ...context("test"), generation: "selected-generation" };
  accountState.user = selected;
  finish(status);
  await pending;
  expect(accountState.user).toEqual(selected);
  expect(invoke).toHaveBeenCalledTimes(1);
});

it.each(["host", "registration"] as const)(
  "retired %s completion cannot read or replace a new owner account",
  async (kind) => {
    accountState.user = context("device");
    let release!: (value: unknown) => void;
    vi.mocked(invoke).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );
    const pending =
      kind === "host"
        ? nativeHost()!.restart(1)
        : nativeService()!.requestRegistration();
    retireAccountOperations();
    accountState.user = { ...context("device"), generation: "new-owner" };
    release(status);
    await pending;
    expect(accountState.user?.generation).toBe("new-owner");
    expect(vi.mocked(invoke).mock.calls).toHaveLength(1);
  },
);
