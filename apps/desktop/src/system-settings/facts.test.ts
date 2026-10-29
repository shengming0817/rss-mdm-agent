import { afterAll, afterEach, expect, it } from "vitest";
import type { CollectedData, QueryDescriptor } from "../self-service/types";
import { createSystemSettings } from "./controller";
import { homeFacts } from "./facts";
const controller = createSystemSettings(null, null);
afterEach(() => {
  controller.state.facts.clear();
  controller.state.observations.clear();
});
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
      collectedAt: Math.floor(controller.state.now / 1000),
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

it("reports the age of the APFS container that supplied capacity", () => {
  put("volume", "hardware.volumes", [
    { system_volume: true, container_id: "shared" },
  ]);
  put("container", "hardware.containers", [
    { id: "shared", size_bytes: 1000, free_bytes: 500 },
  ]);
  controller.state.facts.get("container")!.value.stale = true;
  expect(homeFacts(controller).storageDetail).toContain("观察已过期");
});

it("reports old native power state even when the battery collection is fresh", () => {
  put("battery", "hardware.battery", [], {
    "hardware.battery.charge_percent": 50,
  });
  controller.state.observations.set("power", {
    writeAllowed: false,
    request: null,
    digest: null,
    finished: true,
    effect: "observed",
    keepBeforeUnixMs: null,
    reason: null,
    snapshot: {
      section: "power",
      revision: "a".repeat(64),
      collectedAtUnixMs: controller.state.now - 60000,
      source: "native fixture",
      targets: [
        {
          id: "power-source",
          name: "Power",
          options: [],
          reason: null,
          fields: [
            {
              key: "source",
              value: "AC",
              quality: "complete",
              writable: false,
              reason: null,
            },
          ],
        },
      ],
    },
  });
  expect(homeFacts(controller).battery).toBe("50%");
  expect(homeFacts(controller).batteryDetail).toContain("供电状态已过期");
});

it("keeps each page attachment within its own source clock and native observation deadline", () => {
  put("volume", "hardware.volumes", [
    { system_volume: true, size_bytes: 1000, free_bytes: 500 },
  ]);
  let facts = homeFacts(controller);
  expect(facts.storageContext.verified).toBe(true);
  expect(facts.networkContext.collectedAt).toBeUndefined();
  expect(facts.networkContext.verified).toBe(false);
  expect(facts.powerContext.verified).toBe(false);
  put("network", "hardware.network", [{ addresses: ["192.0.2.1"] }]);
  controller.state.observations.set("network", {
    snapshot: {
      section: "network",
      revision: "test",
      collectedAtUnixMs: controller.state.now - 45000,
      source: "native",
      targets: [
        { id: "active-network", fields: [{ key: "interface", value: "en0" }] },
      ],
    },
  } as never);
  facts = homeFacts(controller);
  expect(facts.networkContext.verified).toBe(false);
  expect(facts.networkContext.validUntil).toBe(controller.state.now - 15000);
  expect(facts.storageContext.verified).toBe(true);
  controller.state.observations.get("network")!.snapshot!.collectedAtUnixMs =
    controller.state.now + 1;
  expect(homeFacts(controller).networkContext.verified).toBe(false);
});
