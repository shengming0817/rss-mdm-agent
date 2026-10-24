import { afterAll, afterEach, expect, it } from "vitest";
import type { CollectedData, QueryDescriptor } from "../self-service/types";
import { createSystemSettings } from "./controller";
import { homeFacts } from "./facts";
const controller = createSystemSettings(null, null);
afterEach(() => controller.state.facts.clear());
afterAll(() => controller.dispose());
function put(
  id: string,
  domain: string,
  rows: Record<string, unknown>[],
  fields = {},
) {
  controller.state.facts.set(id, {
    source: { domains: [domain] } as QueryDescriptor,
    value: {
      rows: rows.map((values) => ({ values })),
      fields,
      quality: "complete",
      stale: false,
    } as unknown as CollectedData,
  });
}
it("does not fabricate a disk total or battery for absent observations", () => {
  expect(homeFacts(controller)).toMatchObject({
    storageFree: "—",
    storageUsed: undefined,
    battery: "—",
    batteryPercent: undefined,
  });
});
it("keeps a real zero charge and zero free space", () => {
  put("battery", "hardware.battery", [], {
    "hardware.battery.charge_percent": 0,
  });
  put("volume", "hardware.volumes", [
    { system_volume: true, size_bytes: 1000 ** 3, free_bytes: 0 },
  ]);
  expect(homeFacts(controller)).toMatchObject({
    storageFree: "0.0",
    storageUsed: 100,
    battery: "0%",
    batteryPercent: 0,
  });
});
it("uses the system volume's shared APFS container without summing unrelated disks", () => {
  put("volume", "hardware.volumes", [
    {
      system_volume: true,
      container_id: "container",
      size_bytes: 10,
      free_bytes: 1,
    },
    { size_bytes: 10000, free_bytes: 9000 },
  ]);
  put("containers", "hardware.containers", [
    {
      id: "container",
      size_bytes: 100 * 1000 ** 3,
      free_bytes: 25 * 1000 ** 3,
    },
  ]);
  expect(homeFacts(controller)).toMatchObject({
    storageFree: "25.0",
    storageUsed: 75,
  });
  expect(homeFacts(controller).storageDetail).toContain("系统卷所属容器");
});
