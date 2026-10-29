import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";

import {
  currentUser,
  accountNotice,
  accountBinding,
  refreshAccount,
  logoutAccount,
  accountErrorMessage,
  loadAccountSettings,
  type Organization,
} from "./test-users";

import { createHostSettings } from "./settings/controller";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));

const settingsPage = (
  organizations: Organization[] = [],
  selected?: string,
) => ({
  schemaVersion: 1,
  kind: "accountSettings",
  organizations,
  ...(selected ? { selected } : {}),
});

const current = {
  schemaVersion: 1 as const,
  kind: "userContext" as const,
  user: {
    schemaVersion: 1 as const,
    kind: "testUser" as const,
    userId: "a",
    displayName: "Alice",
    nameKey: "alice",
  },
  generation: "generation-a",
  identity: {
    mode: "device" as const,
    authorityId: "desktop-test",
    tenantId: "local-test",
    principalId: "a",
  },
};

beforeEach(() => {
  currentUser.current = undefined;
  accountNotice.current = "";
  accountBinding.current = "unbound";
  vi.mocked(invoke).mockReset();
});

it("installs the native device workspace after logout retires the old generation", async () => {
  currentUser.current = { ...current, generation: "old-enterprise-generation" };
  const device = { ...current, generation: "new-device-generation" };
  vi.mocked(invoke).mockImplementation(async (command) => {
    expect(currentUser.current).toBeUndefined();
    if (command === "account_logout") return;
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        current: device,
      };
    throw Error("unexpected native request");
  });
  await logoutAccount();
  expect(currentUser.current).toEqual(device);
  expect(currentUser.current?.identity.mode).toBe("device");
});

it("keeps the retired identity unavailable when logout cleanup is unconfirmed", async () => {
  currentUser.current = current;
  vi.mocked(invoke).mockRejectedValue({ code: "logout_unconfirmed" });
  await expect(logoutAccount()).rejects.toEqual({ code: "logout_unconfirmed" });
  expect(currentUser.current).toBeUndefined();
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
    "account_logout",
    "account_status",
  ]);
});

it("projects closed account errors without exposing native details and rejects invalid IPC settings", async () => {
  expect(
    accountErrorMessage({
      code: "account/login/unavailable",
      message: "SECRET",
    }),
  ).toContain("网络或服务");
  expect(accountErrorMessage({ code: "account/login/rate_limited" })).toContain(
    "限流",
  );
  expect(
    accountErrorMessage({ code: "account/authorization/denied" }),
  ).toContain("业务授权");
  expect(
    accountErrorMessage({ code: "account/configuration/configuration" }),
  ).toContain("租户 UUID");
  expect(
    accountErrorMessage({ code: "unknown", message: "SECRET" }),
  ).not.toContain("SECRET");
  vi.mocked(invoke).mockResolvedValue({
    ...settingsPage(),
    organizations: [
      {
        id: "x",
        label: "x",
        origin: "https://example.com",
        tenantId: "tenant",
        password: "SECRET",
      },
    ],
  });
  await expect(loadAccountSettings()).rejects.toThrow();
});

it("ignores late account binding declarations after the native user generation changes", async () => {
  currentUser.current = current;
  let release!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const read = refreshAccount();
  currentUser.current = { ...current, generation: "replacement" };
  accountBinding.current = "bound";
  release({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "invalidated",
    current,
  });
  await read;
  expect(currentUser.current?.generation).toBe("replacement");
  expect(accountBinding.current).toBe("bound");
});
it.each(["cleanup_pending", "conflict"] as const)(
  "keeps native recovery binding after rejected logout: %s",
  async (binding) => {
    currentUser.current = current;
    accountBinding.current = "bound";
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "account_logout") throw { code: "cleanup_pending" };
      if (command === "account_status")
        return { schemaVersion: 1, kind: "accountStatus", binding };
      throw Error("unexpected request");
    });
    await expect(logoutAccount()).rejects.toEqual({ code: "cleanup_pending" });
    expect(currentUser.current).toBeUndefined();
    expect(accountBinding.current).toBe(binding);
  },
);
