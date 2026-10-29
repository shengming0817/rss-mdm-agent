import { expect, it, vi } from "vitest";
import { createNetworkCheck } from "./controller";
import { executionTask } from "../../../../tests/self-service/support";
import type { NetworkCheckPort } from "../self-service/native";
it("defaults are read-only and a lost result recovers the original frozen target without another run", async () => {
  const target = { host: "service.example", port: 443 },
    port = vi.fn<NetworkCheckPort>();
  port.mockResolvedValueOnce({ kind: "defaults", value: target });
  const owner = createNetworkCheck(port, () => "original");
  await owner.load();
  expect(port).toHaveBeenCalledExactlyOnceWith({ kind: "defaults" });
  port.mockRejectedValueOnce(Error("lost"));
  await owner.run(target);
  target.host = "changed.example";
  await owner.run(target);
  expect(port).toHaveBeenCalledTimes(2);
  expect(owner.state.target?.host).toBe("service.example");
  port.mockResolvedValueOnce({
    kind: "status",
    value: {
      status: {
        ...executionTask().status,
        operationRequestId: "original",
        phase: "executionEnded",
      },
      result: null,
    },
  });
  await owner.recover();
  expect(port).toHaveBeenLastCalledWith({
    kind: "status",
    request: "original",
  });
  expect(owner.ended()).toBe(true);
  owner.dispose();
});
it("does not accept another request's receipt or publish after the workspace retires", async () => {
  let complete!: (value: Awaited<ReturnType<NetworkCheckPort>>) => void;
  const owner = createNetworkCheck(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
    () => "original",
  );
  const pending = owner.run({ host: "example.test", port: 443 });
  owner.dispose();
  complete({
    kind: "status",
    value: {
      status: { ...executionTask().status, operationRequestId: "other" },
      result: null,
    },
  });
  await pending;
  expect(owner.state.status).toBeNull();
  expect(owner.state.request).toBeNull();
});
it("allows correction after explicit invalid input, while unknown delivery resends the same request and target", async () => {
  const port = vi
      .fn<NetworkCheckPort>()
      .mockRejectedValueOnce({ code: "network_invalid_input" }),
    identity = vi
      .fn()
      .mockReturnValueOnce("rejected")
      .mockReturnValue("original");
  const owner = createNetworkCheck(port, identity);
  await owner.run({ host: "-bad.example", port: 443 });
  expect(owner.state.request).toBeNull();
  port.mockRejectedValueOnce(Error("offline"));
  await owner.run({ host: "service.example", port: 443 });
  port.mockRejectedValueOnce({ code: "network_not_found" });
  await owner.recover();
  port.mockResolvedValueOnce({
    kind: "status",
    value: {
      status: {
        ...executionTask().status,
        operationRequestId: "original",
        phase: "executionEnded",
      },
      result: null,
    },
  });
  await owner.resend();
  expect(port).toHaveBeenLastCalledWith({
    kind: "run",
    request: "original",
    target: { host: "service.example", port: 443 },
  });
  expect(identity).toHaveBeenCalledTimes(2);
  owner.dispose();
});
