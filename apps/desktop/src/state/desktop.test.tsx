import * as assistantNative from "../assistant/native";
import type { RuntimeClient, SessionView } from "@rss-mdm-agent/ai-client";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
import { activeStage } from "@rss-mdm-agent/ai-contract";
import { StrictMode, useEffect } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useSnapshot } from "valtio";
import { accountState, retireAccountOperations } from "../test-users";
import { taskPage } from "../../../../tests/self-service/support";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import { createDesktop } from "./desktop";
import { App } from "../App";
import { DesktopProvider, useDesktop, useWorkspace } from "./react";
import { workspaceIdentity } from "../workspace/identity";
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
const user: UserContext = {
  schemaVersion: 1,
  kind: "userContext",
  generation: "generation-a",
  user: {
    schemaVersion: 1,
    kind: "testUser",
    userId: "a",
    displayName: "Alice",
    nameKey: "alice",
  },
  identity: {
    mode: "device",
    authorityId: "desktop-test",
    tenantId: "local-test",
    principalId: "a",
  },
};
const owners: ReturnType<typeof createDesktop>[] = [];
const ai = new Map<string, (id: string) => void>();
const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
let readStatus = async () => ({
  schemaVersion: 1,
  kind: "accountStatus",
  binding: "unbound",
  current: user,
});
beforeEach(() => {
  vi.useFakeTimers();
  retireAccountOperations();
  vi.mocked(invoke).mockReset();
  ai.clear();
  readStatus = async () => ({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: user,
  });
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    switch (command) {
      case "test_users":
        return {
          schemaVersion: 1,
          kind: "testUserPage",
          users: [user.user],
          current: user,
        };
      case "account_status":
        return readStatus();
      case "ai_connect":
        return new Promise<string>((resolve) =>
          ai.set(
            String(args && "generation" in args ? args.generation : ""),
            resolve,
          ),
        );
      case "ai_disconnect":
        return;
      case "task_list":
        return taskPage([]);
      case "appearance_snapshot":
        return {
          materialEnabled: false,
          reducedMotion: false,
          highContrast: false,
        };
      default:
        throw Error("unconfigured native read");
    }
  });
});
afterEach(() => {
  for (const owner of owners.splice(0)) owner.dispose();
  retireAccountOperations();
  vi.useRealTimers();
});
function desktop() {
  const owner = createDesktop();
  owners.push(owner);
  return owner;
}
it("publishes only the verified startup identity and binds original IPC clients once", async () => {
  const verified = deferred<Awaited<ReturnType<typeof readStatus>>>();
  readStatus = () => verified.promise;
  const owner = desktop(),
    starting = owner.start();
  await Promise.resolve();
  await Promise.resolve();
  expect(accountState.user).toEqual(user);
  expect(owner.state.workspace).toBeUndefined();
  expect(ai.size).toBe(0);
  verified.resolve({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: user,
  });
  await starting;
  const original = owner.state.workspace!;
  expect(original.key).toBe(workspaceIdentity(user));
  expect(
    vi.mocked(invoke).mock.calls.filter(([c]) => c === "ai_connect"),
  ).toEqual([["ai_connect", { generation: user.generation }]]);
  original.assistant.draft.set("private draft");
  original.navigation.select("software", "updates");
  for (let i = 0; i < 5; i++) original.navigation.cycle();
  owner.setTheme("dark");
  await owner.start();
  expect(owner.state.workspace).toBe(original);
  expect(original.assistant.draft.get()).toBe("private draft");
  expect(ai.size).toBe(1);
  owner.dispose();
  ai.get(user.generation)!("late-connection");
  await vi.advanceTimersByTimeAsync(0);
  expect(invoke).toHaveBeenCalledWith("ai_disconnect", {
    connectionId: "late-connection",
  });
  expect(vi.getTimerCount()).toBe(0);
  expect(
    vi
      .mocked(invoke)
      .mock.calls.some(
        ([c]) => c === "task_control" || c === "self_service_execute",
      ),
  ).toBe(false);
});
it.each(["generation", "tenant", "principal", "authority", "binding"])(
  "retires the entire previous workspace when %s changes",
  async (coordinate) => {
    const owner = desktop();
    await owner.start();
    const first = owner.state.workspace!;
    first.assistant.draft.set("PRIVATE");
    first.navigation.select("software", "updates");
    const next = structuredClone(user);
    if (coordinate === "generation") next.generation = "generation-b";
    else if (coordinate === "tenant") next.identity.tenantId = "next";
    else if (coordinate === "principal") next.identity.principalId = "next";
    else if (coordinate === "authority") next.identity.authorityId = "next";
    else next.executionBinding = "device-binding-b";
    accountState.user = next;
    expect(owner.state.workspace?.key).toBe(workspaceIdentity(next));
    expect(owner.state.workspace).not.toBe(first);
    expect(first.assistant.state.drafts.size).toBe(0);
    expect(first.assistant.state.connection).toBe("disconnected");
    expect(owner.state.workspace?.assistant.draft.get()).toBe("");
    expect(owner.state.workspace?.navigation.state.selected.software).toBe(
      "discover",
    );
  },
);
it("keeps a lost registration response isolated from passive account reads", async () => {
  const owner = desktop();
  await owner.start();
  const first = owner.state.workspace!;
  const registration = deferred<unknown>();
  const stale = deferred<Awaited<ReturnType<typeof readStatus>>>();
  readStatus = () => stale.promise;
  const passive = owner.pollAccount();
  vi.mocked(invoke).mockImplementationOnce(() => registration.promise);
  const pending = owner.service!.requestRegistration();
  expect(owner.state.workspace).toBeUndefined();
  stale.resolve({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: { ...user, generation: "stale" },
  });
  await passive;
  expect(accountState.user?.generation).toBe(user.generation);
  readStatus = async () => ({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: { ...user, generation: "registered" },
  });
  registration.resolve({});
  await pending;
  expect(first.assistant.state.drafts.size).toBe(0);
  expect(owner.state.workspace?.identity.generation).toBe("registered");
});
it("StrictMode remount closes previous owners and subscriptions without reconnecting on appearance renders", async () => {
  const mounted: ReturnType<typeof createDesktop>[] = [];
  let current: ReturnType<typeof createDesktop> | undefined;
  function Probe() {
    const owner = useDesktop();
    const workspace = useWorkspace();
    const state = useSnapshot(owner.state);
    current = owner;
    useEffect(() => {
      mounted.push(owner);
    }, [owner]);
    return (
      <output>
        {state.phase}:{workspace?.key}
      </output>
    );
  }
  const app = render(
    <StrictMode>
      <DesktopProvider>
        <Probe />
      </DesktopProvider>
    </StrictMode>,
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(screen.getByRole("status").textContent).toContain("ready:");
  const workspace = current!.state.workspace;
  await act(async () => {
    current!.setTheme("dark");
  });
  expect(current!.state.workspace).toBe(workspace);
  app.unmount();
  await vi.advanceTimersByTimeAsync(0);
  expect(current!.state.workspace).toBeUndefined();
  expect(vi.getTimerCount()).toBe(0);
  expect(new Set(mounted).size).toBeGreaterThanOrEqual(1);
  const count = vi.mocked(invoke).mock.calls.length;
  accountState.user = { ...user, generation: "after-unmount" };
  await vi.advanceTimersByTimeAsync(60_000);
  expect(vi.mocked(invoke).mock.calls).toHaveLength(count);
});

it("fails closed when startup account verification fails and never activates fixture clients", async () => {
  readStatus = async () => {
    throw Error("unavailable");
  };
  const owner = desktop();
  await owner.start();
  expect(owner.state.phase).toBe("failed");
  expect(owner.state.workspace).toBeUndefined();
  expect(ai.size).toBe(0);
  expect(
    vi.mocked(invoke).mock.calls.some(([c]) => c === "select_device"),
  ).toBe(false);
});
it("a retired startup response cannot replace the next mounted owner's account", async () => {
  const page = deferred<unknown>();
  vi.mocked(invoke)
    .mockImplementationOnce(async () => ({
      materialEnabled: false,
      reducedMotion: false,
      highContrast: false,
    }))
    .mockImplementationOnce(() => page.promise);
  const previous = desktop(),
    pending = previous.start();
  previous.dispose();
  const next = desktop();
  await next.start();
  const workspace = next.state.workspace;
  page.resolve({
    schemaVersion: 1,
    kind: "testUserPage",
    users: [],
    current: { ...user, generation: "late-startup" },
  });
  await pending;
  expect(next.state.workspace).toBe(workspace);
  expect(accountState.user?.generation).toBe(user.generation);
});

it("retries a failed startup in the same owner without duplicating polling or listeners", async () => {
  readStatus = async () => {
    throw Error("temporary");
  };
  const owner = desktop();
  await owner.start();
  expect(owner.state.phase).toBe("failed");
  expect(vi.getTimerCount()).toBe(0);
  readStatus = async () => ({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: user,
  });
  await owner.start();
  expect(owner.state.phase).toBe("ready");
  const workspace = owner.state.workspace;
  const timers = vi.getTimerCount();
  await Promise.all([owner.start(), owner.start()]);
  expect(owner.state.workspace).toBe(workspace);
  expect(vi.getTimerCount()).toBe(timers);
  const before = vi
    .mocked(invoke)
    .mock.calls.filter(([c]) => c === "account_status").length;
  await vi.advanceTimersByTimeAsync(2000);
  expect(
    vi.mocked(invoke).mock.calls.filter(([c]) => c === "account_status"),
  ).toHaveLength(before + 1);
  owner.dispose();
  await vi.advanceTimersByTimeAsync(0);
  expect(vi.getTimerCount()).toBe(0);
});
it("mounts a new connected client after unmount and ignores callbacks retained by the old transport", async () => {
  const clients: Array<{
    runtime: RuntimeClient;
    close: ReturnType<typeof vi.fn>;
    stop: ReturnType<typeof vi.fn>;
    publish: (view: SessionView) => void;
  }> = [];
  vi.spyOn(assistantNative, "nativeAssistant").mockImplementation(() => ({
    async connect() {
      const close = vi.fn(),
        stop = vi.fn(),
        client = {
          close,
          stop,
          publish: (_view: SessionView) => {},
          runtime: undefined as unknown as RuntimeClient,
        };
      const runtime = {
        initialize: async () => ({}),
        listSessions: async () => ({ items: [] }),
        connections: async () => ({
          connections: [],
          preferences: { schemaVersion: 1, kind: "userPreferences" },
        }),
        observe: (listener: (view: SessionView) => void) => {
          client.publish = listener;
          return stop;
        },
        close,
        connection: { closed: new Promise(() => {}) },
      } as unknown as RuntimeClient;
      client.runtime = runtime;
      clients.push(client);
      return { runtime, mode: "live" };
    },
  }));
  let current: ReturnType<typeof createDesktop> | undefined;
  function Probe() {
    current = useDesktop();
    return <output>mounted</output>;
  }
  const first = render(
    <DesktopProvider>
      <Probe />
    </DesktopProvider>,
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  const firstOwner = current!,
    oldWorkspace = firstOwner.state.workspace!;
  expect(oldWorkspace.assistant.state.connection).toBe("connected");
  first.unmount();
  expect(clients[0].close).toHaveBeenCalledOnce();
  expect(clients[0].stop).toHaveBeenCalledOnce();
  const second = render(
    <DesktopProvider>
      <Probe />
    </DesktopProvider>,
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(current).not.toBe(firstOwner);
  expect(clients).toHaveLength(2);
  expect(clients[1].runtime).not.toBe(clients[0].runtime);
  const workspace = current!.state.workspace!;
  expect(workspace).not.toBe(oldWorkspace);
  expect(workspace.assistant.state.connection).toBe("connected");
  const session = fixtureSession();
  const late: SessionView = {
    namespace: session.namespace,
    generation: activeStage(session).binding.generation,
    cursor: 0,
    capabilities: activeStage(session).capabilities,
    sessionStatus: "active",
    connection: "attached",
    timeline: [],
    commands: {},
    messages: {},
    interactions: {},
    surfaces: {},
    tools: {},
    deliveries: {},
  };
  clients[0].publish(late);
  expect(workspace.assistant.state.views.size).toBe(0);
  expect(oldWorkspace.assistant.state.views.size).toBe(0);
  second.unmount();
  expect(clients[1].close).toHaveBeenCalledOnce();
  expect(clients[1].stop).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
});

it("the production App exposes a working retry after a temporary startup failure", async () => {
  readStatus = async () => {
    throw Error("temporary");
  };
  const app = render(
    <DesktopProvider>
      <App />
    </DesktopProvider>,
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(screen.getByRole("button", { name: "重新读取" })).toBeTruthy();
  readStatus = async () => ({
    schemaVersion: 1,
    kind: "accountStatus",
    binding: "unbound",
    current: user,
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(screen.getByText("本机会话已读取，业务页面正在重建。")).toBeTruthy();
  app.unmount();
  await vi.advanceTimersByTimeAsync(0);
});
