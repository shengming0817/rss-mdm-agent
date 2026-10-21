import { expect, it, vi } from "vitest";
import { createDeviceOverview } from "./device";
import type { DeviceOverview } from "../self-service/types";
const sample: DeviceOverview = {
  attributes: {
    computerName: "Studio Mac",
    ipAddresses: ["192.0.2.24"],
    loginUser: "studio",
  },
};
it("clears obsolete device facts after a failed read and recovers", async () => {
  const read = vi
    .fn()
    .mockResolvedValueOnce(sample)
    .mockRejectedValueOnce(new Error("unavailable"))
    .mockResolvedValueOnce({
      ...sample,
      attributes: { ...sample.attributes, ipAddresses: [], loginUser: null },
    });
  const c = createDeviceOverview({ read });
  await c.refresh();
  expect(c.state.value).toEqual(sample);
  await c.refresh();
  expect(c.state.value).toBeNull();
  await c.refresh();
  expect(c.state.value?.attributes.ipAddresses).toEqual([]);
  expect(c.state.value?.attributes.loginUser).toBeNull();
  c.dispose();
});
it("coalesces overlapping reads and ignores a late response after disposal", async () => {
  let finish!: (value: DeviceOverview) => void;
  const read = vi.fn(
    () =>
      new Promise<DeviceOverview>((resolve) => {
        finish = resolve;
      }),
  );
  const c = createDeviceOverview({ read });
  const pending = c.refresh();
  await c.refresh();
  expect(read).toHaveBeenCalledTimes(1);
  c.dispose();
  finish(sample);
  await pending;
  await c.refresh();
  expect(c.state.value).toBeNull();
  expect(read).toHaveBeenCalledTimes(1);
});
it("keeps an ordinary browser disconnected instead of supplying sample values", async () => {
  const c = createDeviceOverview();
  await c.refresh();
  expect(c.state.value).toBeNull();
  expect(c.state.loading).toBe(false);
});
