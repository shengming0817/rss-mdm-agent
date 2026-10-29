import { expect, it, vi } from "vitest";
import { createRegistration } from "./registration";
import { createOrganizations } from "./organizations";
import { createLoginFixture } from "../../../../tests/assistant/login-fixture";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
const user: UserContext = {
  schemaVersion: 1,
  kind: "userContext",
  generation: "test",
  user: {
    schemaVersion: 1,
    kind: "testUser",
    userId: "person",
    displayName: "person",
    nameKey: "person",
  },
  identity: {
    mode: "device",
    authorityId: "fixture",
    tenantId: "local",
    principalId: "person",
  },
};
it("projects service, registration and MDM independently and clears failed observations", async () => {
  const service = {
      read: vi.fn(async () => ({
        phase: "connected" as const,
        status: {
          build: "test",
          platform: "macos",
          protocol: 8,
          readiness: { phase: "ready" as const },
        },
      })),
      registration: vi.fn(async () => ({
        phase: "activationUnknown" as const,
        registered: false,
        credential: "active" as const,
      })),
      mdmStatus: vi.fn(async () => ({ state: "unknown" as const })),
      requestRegistration: vi.fn(),
      cancelRegistration: vi.fn(),
      importRegistration: vi.fn(),
    },
    model = createRegistration(service);
  await model.refresh();
  expect(model.state.service?.phase).toBe("connected");
  expect(model.state.registration?.registered).toBe(false);
  expect(model.state.registration?.phase).toBe("activationUnknown");
  expect(model.state.mdm?.state).toBe("unknown");
  service.registration.mockRejectedValueOnce(Error("PRIVATE"));
  await model.refresh();
  expect(model.state.registration).toBeUndefined();
  expect(model.state.registrationError).not.toContain("PRIVATE");
  expect(service.requestRegistration).not.toHaveBeenCalled();
  model.dispose();
});
it("late organization and registration reads cannot repopulate a disposed view", async () => {
  let release!: (page: never) => void;
  const load = vi.fn(
      () =>
        new Promise<never>((resolve) => {
          release = resolve;
        }),
    ),
    model = createOrganizations({ load, save: vi.fn() });
  const pending = model.load();
  model.dispose();
  release({
    schemaVersion: 1,
    kind: "accountSettings",
    organizations: [
      {
        id: "a",
        label: "PRIVATE",
        origin: "https://example.com",
        tenantId: "tenant",
      },
    ],
  } as never);
  await pending;
  expect(model.state.items).toEqual([]);
});
it("retires QR expiry and discovery timers on close and cannot finish a cancelled login", async () => {
  vi.useFakeTimers();
  const identity = vi.fn(async () => user),
    port = createLoginFixture(identity);
  port.refreshQr();
  port.scan();
  const pending = port.confirm();
  port.cancel();
  await vi.advanceTimersByTimeAsync(0);
  await pending;
  expect(identity).not.toHaveBeenCalled();
  expect(port.state.current).toBeUndefined();
  expect(port.state.error).toBe("");
  expect(vi.getTimerCount()).toBe(0);
  vi.useRealTimers();
});
it("passes organization and account into the explicit login authority and refuses stale client states", async () => {
  vi.useFakeTimers();
  const identity = vi.fn(async () => user),
    port = createLoginFixture(identity);
  port.organization("beichen");
  const login = port.password({
    organizationId: "beichen",
    login: "person",
    password: "SECRET",
  });
  await vi.advanceTimersByTimeAsync(250);
  await login;
  expect(identity).toHaveBeenCalledWith("beichen", "person");
  port.scenario("mismatch");
  await vi.advanceTimersByTimeAsync(250);
  await port.loginClient("lark");
  expect(identity).toHaveBeenCalledOnce();
  port.cancel();
  expect(vi.getTimerCount()).toBe(0);
  vi.useRealTimers();
});

it("settles a synchronous unavailable port independently and releases the loading state", async () => {
  const model = createRegistration({
    read() {
      throw Error("private transport failure");
    },
    registration: async () => ({
      phase: "unregistered",
      registered: false,
      credential: "absent",
    }),
    mdmStatus: async () => ({ state: "unknown" }),
    requestRegistration: vi.fn(),
    cancelRegistration: vi.fn(),
    importRegistration: vi.fn(),
  });
  await model.refresh();
  expect(model.state.loading).toBe(false);
  expect(model.state.service).toBeUndefined();
  expect(model.state.serviceError).toBe("无法读取本机服务状态");
  expect(model.state.registration?.phase).toBe("unregistered");
  model.dispose();
});
