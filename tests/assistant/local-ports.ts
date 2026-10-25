// Explicit sample ports; the product pages and controllers are identical to production.
import type { LocalPorts } from "../../apps/desktop/src/assembly";
import type {
  CollectedData,
  QueryDescriptor,
  InstalledSoftware,
} from "../../apps/desktop/src/self-service/types";
export function fixtureLocalPorts(): LocalPorts {
  const revision = "1".repeat(64),
    snapshot = "2".repeat(64);
  const domains = [
    "software.installations",
    "hardware.network",
    "hardware.volumes",
    "hardware.battery",
  ];
  const descriptors: QueryDescriptor[] = domains.map((domain) => ({
    reference: { id: domain, revision },
    available: true,
    domains: [domain],
    inputSchema: {},
    kind: "native",
    origin: "local",
    policy: null,
    reason: null,
    scope: { kind: "device" },
    title: domain,
  }));
  const items: InstalledSoftware[] = [
    ["workspace", "Workspace", "4.2"],
    ["notes", "Notes", "2.1"],
    ["browser", "Browser", "8.0"],
    ["sync", "Work Sync", "3.0"],
  ].map(([id, name, version]) => ({
    coverage: "4".repeat(64),
    reference: {
      query: descriptors[0].reference,
      cursor: { snapshot, offset: 0 },
      installation: id,
    },
    kind: "application",
    name,
    version,
    nativeId: `org.fixture.${id}`,
    origin: "application",
    scope: { kind: "device" },
    publisher: "Fixture",
    actions: [
      {
        action: "launch",
        available: false,
        reason: "unsupported",
        targetVersion: null,
      },
    ],
  }));
  const unsupported = async (): Promise<never> => {
    throw Error("fixture capability not configured");
  };
  return {
    data: {
      query_directory: async () => ({
        items: descriptors,
        next: null,
        policyNext: null,
      }),
      data_read: async (input): Promise<CollectedData> => {
        if (input.selection.kind !== "query") throw Error("fixture selection");
        const query = input.selection.query;
        return {
          query,
          scope: { kind: "device" },
          collectedAt: Math.floor(Date.now() / 1000),
          enumerationComplete: true,
          next: null,
          quality: "complete",
          request: null,
          stale: false,
          fields:
            query.id === "hardware.battery"
              ? { "hardware.battery.charge_percent": 86 }
              : {},
          rows:
            query.id === "hardware.network"
              ? [{ values: { addresses: ["192.0.2.24"] } }]
              : query.id === "hardware.volumes"
                ? [
                    {
                      values: {
                        system_volume: true,
                        size_bytes: 512000000000,
                        free_bytes: 186000000000,
                      },
                    },
                  ]
                : [],
        };
      },
      query_submit: unsupported,
      query_status: unsupported,
      query_cancel: unsupported,
    },
    software: {
      read: async () => ({
        query: descriptors[0].reference,
        coverage: ["4".repeat(64)],
        collectedAt: Math.floor(Date.now() / 1000),
        quality: "complete",
        enumerationComplete: true,
        stale: false,
        items,
        next: null,
      }),
      history: async () => ({ items: [], next: null }),
      prepare: unsupported,
      submit: unsupported,
      status: unsupported,
      cancel: unsupported,
    },
    settings: async () => ({
      request: null,
      digest: null,
      finished: true,
      effect: "observed",
      snapshot: null,
      keepBeforeUnixMs: null,
      reason: "显式样本装配未提供原生设置",
      writeAllowed: false,
    }),
    network: null,
  };
}
