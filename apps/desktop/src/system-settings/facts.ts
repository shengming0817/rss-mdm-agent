import type { Snapshot } from "valtio/vanilla";
import type { CollectedData, SettingsSnapshot } from "../self-service/types";
import type { SystemSettingsController } from "./controller";
const number = (v: unknown) =>
  typeof v === "number" && Number.isFinite(v)
    ? v
    : typeof v === "string" && /^\d+$/.test(v)
      ? Number(v)
      : undefined;
function rows(value?: Snapshot<CollectedData>): Record<string, unknown>[] {
  return (
    value?.rows.flatMap((row) =>
      row &&
      typeof row === "object" &&
      !Array.isArray(row) &&
      "values" in row &&
      row.values &&
      typeof row.values === "object" &&
      !Array.isArray(row.values)
        ? [row.values as Record<string, unknown>]
        : [],
    ) ?? []
  );
}
const qualityLabels: Record<string, string> = {
  partial: "部分结果",
  truncated: "结果已截断",
  failed: "采集失败",
  unsupported: "不支持",
  denied: "无权读取",
  empty: "已采集，结果为空",
  mappingUnsupported: "映射不支持",
};
const quality = (value?: Snapshot<CollectedData>) =>
  !value || value.quality === "notCollected"
    ? "尚未采集"
    : value.stale
      ? "观察已过期"
      : value.quality === "complete"
        ? "已采集"
        : (qualityLabels[value.quality] ?? "结果未知");
const collectedTime = (value?: Snapshot<CollectedData>) =>
  value?.collectedAt == null
    ? ""
    : ` · ${new Date(value.collectedAt * 1000).toLocaleTimeString()}`;
// Evidence follows the same inputs as each display projection; never borrow another resource's clock.
function contextEvidence(
  values: (Snapshot<CollectedData> | undefined)[],
  native: Snapshot<SettingsSnapshot> | null | undefined,
  now: number,
) {
  const samples = values.filter((value) => value !== undefined);
  const times = samples.map((value) =>
    value.collectedAt == null ? NaN : value.collectedAt * 1000,
  );
  const deadlines = times.map((at) => at + 300000);
  if (native) {
    times.push(native.collectedAtUnixMs);
    deadlines.push(native.collectedAtUnixMs + 30000);
  }
  const known =
    times.length > 0 && times.every((at) => Number.isFinite(at) && at <= now);
  return {
    collectedAt: known ? Math.min(...times) : undefined,
    validUntil: known ? Math.min(...deadlines) : undefined,
    verified:
      known &&
      values.every((value) => value?.quality === "complete" && !value.stale) &&
      deadlines.every((at) => at > now),
  };
}
export function homeFacts(
  controller?: SystemSettingsController,
  current:
    | Snapshot<SystemSettingsController["state"]>
    | undefined = controller?.state,
  now = current?.now ?? Date.now(),
) {
  const facts = controller?.summary(current);
  const network = facts?.network,
    storage = facts?.storage,
    power = facts?.power;
  const observations = current?.observations;
  const nativeTime = (section: "network" | "power", label: string) => {
    const snapshot = observations?.get(section)?.snapshot;
    if (!snapshot) return "";
    const age = now - snapshot.collectedAtUnixMs;
    return `${label}${age < 0 ? "时间待核实" : age >= 30_000 ? "已过期" : "已读取"} · ${new Date(snapshot.collectedAtUnixMs).toLocaleTimeString()}`;
  };
  const source = observations
    ?.get("power")
    ?.snapshot?.targets.find((t) => t.id === "power-source");
  const batteryPresent = source?.fields.find(
    (f) => f.key === "batteryPresent",
  )?.value;
  const provider = source?.fields.find((f) => f.key === "source")?.value;
  const interfaces = rows(network).filter(
    (r) => Array.isArray(r.addresses) && r.addresses.length,
  );
  const route = observations
    ?.get("network")
    ?.snapshot?.targets.find((t) => t.id === "active-network");
  const routeInterface = route?.fields.find(
    (f) => f.key === "interface",
  )?.value;
  const os = [...(current?.facts.values() ?? [])].find((f) =>
    f.source.domains.includes("hardware.os"),
  )?.value;
  const systemDrive = os?.fields["hardware.os.system_drive"];
  const volume = rows(storage).find(
    (r) =>
      r.system_volume === true ||
      (typeof systemDrive === "string" && r.DeviceID === systemDrive),
  );
  let total = number(volume?.size_bytes ?? volume?.Size),
    free = number(volume?.free_bytes ?? volume?.FreeSpace),
    capacity = storage,
    shared = false;
  if (volume?.container_id) {
    const container = [...(current?.facts.values() ?? [])]
      .filter((f) => f.source.domains.includes("hardware.containers"))
      .flatMap((entry) =>
        rows(entry.value).map((row) => ({ row, value: entry.value })),
      )
      .find((entry) => entry.row.id === volume.container_id);
    if (container) {
      total = number(container.row.size_bytes);
      free = number(container.row.free_bytes);
      capacity = container.value;
      shared = true;
    }
  }
  const charge = Object.entries(power?.fields ?? {}).find(
    ([key]) => key.endsWith(".charge_percent") || key.endsWith(".percentage"),
  )?.[1];
  const percent = number(charge),
    validPercent =
      percent !== undefined && percent >= 0 && percent <= 100
        ? percent
        : undefined;
  const validStorage =
    total !== undefined &&
    free !== undefined &&
    total > 0 &&
    free >= 0 &&
    free <= total;
  return {
    networkContext: contextEvidence(
      [network],
      typeof routeInterface === "string"
        ? observations?.get("network")?.snapshot
        : undefined,
      now,
    ),
    storageContext: contextEvidence(
      [
        storage,
        capacity,
        ...(volume && volume.system_volume !== true ? [os] : []),
      ],
      undefined,
      now,
    ),
    powerContext: contextEvidence(
      [power],
      typeof provider === "string" || typeof batteryPresent === "boolean"
        ? observations?.get("power")?.snapshot
        : undefined,
      now,
    ),
    network:
      typeof routeInterface === "string"
        ? routeInterface
        : network?.quality === "complete"
          ? `${interfaces.length} 个有地址的接口`
          : "未读取",
    networkDetail: `${typeof routeInterface === "string" ? `${nativeTime("network", "路由状态")} · ` : ""}${quality(network)}${collectedTime(network)}`,
    storageFree: validStorage ? (free! / 1000 ** 3).toFixed(1) : "—",
    storageDetail: validStorage
      ? `${shared ? "系统卷所属容器" : "系统卷"} · 共 ${(total! / 1000 ** 3).toFixed(1)} GB · ${quality(capacity)}${collectedTime(capacity)}`
      : `${quality(capacity)}${collectedTime(capacity)}`,
    storageUsed: validStorage
      ? Math.round((1 - free! / total!) * 100)
      : undefined,
    battery:
      batteryPresent === false
        ? "无电池"
        : validPercent !== undefined
          ? `${Math.round(validPercent)}%`
          : "—",
    batteryDetail: `${typeof provider === "string" ? `${provider} · ${nativeTime("power", "供电状态")} · ` : ""}${quality(power)}${collectedTime(power)}`,
    batteryPercent: batteryPresent !== false ? validPercent : undefined,
  };
}
