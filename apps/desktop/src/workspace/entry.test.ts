import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { accountState } from "../test-users";
import { nativeNavigation } from "./entry";
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
afterEach(() => {
  accountState.user = undefined;
});
it("binds all native entry reads and acknowledgements to the generation that acquired the port", async () => {
  accountState.user = { generation: "old" } as never;
  const port = nativeNavigation()!;
  accountState.user = { generation: "new" } as never;
  await port({ kind: "next" });
  await port({ kind: "ack", id: "original" });
  expect(invoke).toHaveBeenNthCalledWith(1, "navigation", {
    generation: "old",
    input: { kind: "next" },
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "navigation", {
    generation: "old",
    input: { kind: "ack", id: "original" },
  });
});
