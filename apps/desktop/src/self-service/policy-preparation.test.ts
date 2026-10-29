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
