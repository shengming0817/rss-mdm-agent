import { beforeEach, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";

import {
  accountState,
  enterGuest,
  loadTestUsers,
  retireAccountOperations,
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
  retireAccountOperations();
  accountState.notice = "";
  accountState.binding = "unbound";
  vi.mocked(invoke).mockReset();
});

it("installs the native device workspace after logout retires the old generation", async () => {
  accountState.user = { ...current, generation: "old-enterprise-generation" };
  const device = { ...current, generation: "new-device-generation" };
  vi.mocked(invoke).mockImplementation(async (command) => {
    expect(accountState.user).toBeUndefined();
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
  expect(accountState.user).toEqual(device);
  expect(accountState.user?.identity.mode).toBe("device");
});

it("keeps the retired identity unavailable when logout cleanup is unconfirmed", async () => {
  accountState.user = current;
  vi.mocked(invoke).mockRejectedValue({ code: "logout_unconfirmed" });
  await expect(logoutAccount()).rejects.toEqual({ code: "logout_unconfirmed" });
  expect(accountState.user).toBeUndefined();
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
  accountState.user = current;
  let release!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const read = refreshAccount();
  accountState.user = { ...current, generation: "replacement" };
  accountState.binding = "bound";
  release({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "invalidated",
    current,
  });
  await read;
  expect(accountState.user?.generation).toBe("replacement");
  expect(accountState.binding).toBe("bound");
});
it.each(["cleanup_pending", "conflict"] as const)(
  "keeps native recovery binding after rejected logout: %s",
  async (binding) => {
    accountState.user = current;
    accountState.binding = "bound";
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "account_logout") throw { code: "cleanup_pending" };
      if (command === "account_status")
        return { schemaVersion: 1, kind: "accountStatus", binding };
      throw Error("unexpected request");
    });
    await expect(logoutAccount()).rejects.toEqual({ code: "cleanup_pending" });
    expect(accountState.user).toBeUndefined();
    expect(accountState.binding).toBe(binding);
  },
);

const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
it("rejects concurrent mutations and discards a successful login after its owner retires", async () => {
  const response = deferred<unknown>();
  vi.mocked(invoke).mockImplementation(() => response.promise);
  const first = enterGuest();
  expect(accountState.busy).toBe(true);
  await expect(enterGuest()).rejects.toThrow("account_busy");
  expect(invoke).toHaveBeenCalledTimes(1);
  retireAccountOperations();
  response.resolve(current);
  await expect(first).rejects.toThrow("account_operation_retired");
  expect(accountState.user).toBeUndefined();
  expect(accountState.busy).toBe(false);
});
it("only accepts the latest passive status and ignores a retired startup page", async () => {
  const startup = deferred<unknown>(),
    older = deferred<unknown>(),
    newer = deferred<unknown>();
  vi.mocked(invoke)
    .mockImplementationOnce(() => startup.promise)
    .mockImplementationOnce(() => older.promise)
    .mockImplementationOnce(() => newer.promise);
  const page = loadTestUsers(),
    a = refreshAccount(),
    b = refreshAccount();
  newer.resolve({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "bound",
    current,
  });
  await b;
  older.resolve({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "invalidated",
  });
  await a;
  startup.resolve({
    schemaVersion: 1,
    kind: "testUserPage",
    users: [],
    current: { ...current, generation: "stale" },
  });
  await page;
  expect(accountState.user).toEqual(current);
  expect(accountState.binding).toBe("bound");
});
