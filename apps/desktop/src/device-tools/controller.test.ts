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
