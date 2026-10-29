import { expect, it, vi } from "vitest";
import { createPolicyPreparation } from "./policy-preparation";
import {
  directoryItem,
  policyFixturePort,
  policyRecord,
} from "../../../../tests/self-service/support";
import type { SelfServicePort } from "./types";
it("resolves without submitting, retains an immutable human request on receipt loss and retries only its original input", async () => {
  const item = directoryItem(policyRecord().action),
    invoke = vi
      .fn()
      .mockRejectedValueOnce(Error("lost"))
      .mockResolvedValue(policyRecord("original"));
  const port: SelfServicePort = {
    ...policyFixturePort,
    execute: vi.fn(),
    confirm: vi.fn(),
    policy_resolve: vi.fn(async () => item),
    policy_invoke: invoke,
    policy_status: vi.fn(async () => ({ kind: "absent" as const })),
  };
  const owner = createPolicyPreparation(port, () => "original");
  await owner.select(item);
  expect(invoke).not.toHaveBeenCalled();
  const parameters = { interface: "en0" };
  await owner.submit(parameters);
  parameters.interface = "changed";
  expect(owner.state.choice?.parameters).toEqual({ interface: "en0" });
  await owner.submit({ interface: "new" });
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(owner.clear()).toBe(false);
  await owner.recover();
  await owner.submit({ interface: "new" });
  expect(invoke).toHaveBeenNthCalledWith(2, {
    choice: {
      request: "original",
      policyId: item.policyId,
      parameters: { interface: "en0" },
    },
  });
  expect(port.policy_status).toHaveBeenCalledWith({ requestId: "original" });
  owner.dispose();
});
it("rejects changed subject/version and ignores a late selection after disposal", async () => {
  const item = directoryItem(policyRecord().action);
  const port: SelfServicePort = {
    ...policyFixturePort,
    execute: vi.fn(),
    confirm: vi.fn(),
    policy_resolve: vi.fn(async () => ({ ...item, policyRevision: 2 })),
  };
  const owner = createPolicyPreparation(port);
  await owner.select(item);
  expect(owner.state.item).toBeNull();
  expect(owner.state.error).toContain("已变化");
  let finish!: (value: typeof item) => void;
  port.policy_resolve = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  const pending = owner.select(item);
  owner.dispose();
  finish(item);
  await pending;
  expect(owner.state.item).toBeNull();
});

it.each(["failed", "cancelled"] as const)(
  "re-resolves the same %s project before preparing changed parameters under a new UUID",
  async (state) => {
    const item = directoryItem(policyRecord().action),
      record = policyRecord("first");
    record.state = state;
    const resolve = vi.fn(async () => item),
      invoke = vi.fn(async () => record);
    let next = 0;
    const owner = createPolicyPreparation(
      {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
        policy_resolve: resolve,
        policy_invoke: invoke,
      },
      () => `request-${++next}`,
    );
    await owner.select(item);
    await owner.submit({ interface: "en0" });
    expect(owner.canRestart()).toBe(true);
    resolve.mockResolvedValue({
      ...item,
      policyVersion: "new-version",
      policyRevision: 2,
    });
    await owner.restart();
    expect(resolve).toHaveBeenCalledTimes(2);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(owner.state.item?.policyVersion).toBe("new-version");
    expect(owner.state.choice).toBeNull();
    await owner.submit({ interface: "en1" });
    expect(invoke).toHaveBeenLastCalledWith({
      choice: {
        request: "request-2",
        policyId: item.policyId,
        parameters: { interface: "en1" },
      },
    });
    owner.dispose();
  },
);
it("does not restart unknown or absent requests, and preserves an ended request when re-resolution fails", async () => {
  const item = directoryItem(policyRecord().action),
    resolve = vi.fn(async () => item);
  const record = policyRecord("original");
  record.state = "failed";
  const owner = createPolicyPreparation(
    {
      ...policyFixturePort,
      execute: vi.fn(),
      confirm: vi.fn(),
      policy_resolve: resolve,
      policy_invoke: vi
        .fn()
        .mockRejectedValueOnce(Error("lost"))
        .mockResolvedValue(record),
      policy_status: async () => ({ kind: "absent" }),
    },
    () => "original",
  );
  await owner.select(item);
  await owner.submit({ value: "original" });
  await owner.restart();
  expect(owner.clear()).toBe(false);
  await owner.recover();
  await owner.restart();
  expect(owner.canRestart()).toBe(false);
  expect(owner.clear()).toBe(false);
  expect(resolve).toHaveBeenCalledTimes(1);
  await owner.submit({ value: "changed" });
  resolve.mockRejectedValueOnce(Error("offline"));
  await owner.restart();
  expect(owner.state.choice?.parameters).toEqual({ value: "original" });
  expect(owner.state.status?.kind).toBe("preparation");
  expect(owner.state.error).toContain("原请求仍可查询");
  owner.dispose();
});

it.each([
  "not_started",
  "running",
  "unknown",
  "waiting_reboot",
  "succeeded",
  "failed",
] as const)(
  "permits a fresh run only after authoritative execution has ended (%s)",
  async (execution) => {
    const item = directoryItem(policyRecord().action);
    const owner = createPolicyPreparation(
      {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
        policy_resolve: async () => item,
        policy_invoke: async () => policyRecord(),
        policy_status: async () => ({
          kind: "run",
          value: {
            requestId: "original",
            runId: "run",
            attemptId: null,
            state: {
              execution,
              delivery: { kind: "queued" },
              cancellation: "none",
              startedAt: null,
              deadline: 100,
            },
            availableAt: 1,
            deadline: 100,
            result: null,
          },
        }),
      },
      () => "original",
    );
    await owner.select(item);
    await owner.submit({});
    await owner.recover();
    expect(owner.canRestart()).toBe(
      ["succeeded", "failed"].includes(execution),
    );
    owner.dispose();
  },
);
it("retains the service-frozen preparation after status moves to a run, and clears it with its original selection", async () => {
  const record = policyRecord("original"),
    item = directoryItem(record.action);
  const owner = createPolicyPreparation(
    {
      ...policyFixturePort,
      execute: vi.fn(),
      confirm: vi.fn(),
      policy_resolve: async () => item,
      policy_invoke: async () => record,
      policy_status: async () => ({
        kind: "run",
        value: {
          requestId: "original",
          runId: "run",
          attemptId: null,
          availableAt: 1,
          deadline: 100,
          result: null,
          state: {
            delivery: { kind: "queued" },
            execution: "succeeded",
            cancellation: "none",
            startedAt: 1,
            deadline: 100,
          },
        },
      }),
    },
    () => "original",
  );
  await owner.select(item);
  await owner.submit({ interface: "en0" });
  await owner.recover();
  expect(owner.state.status?.kind).toBe("run");
  expect(owner.state.preparation?.action).toEqual(record.action);
  expect(owner.clear()).toBe(true);
  expect(owner.state.preparation).toBeNull();
  owner.dispose();
});
