import type { CollectedData } from "../self-service/types";
import type { SystemSettingsController } from "./controller";
const number = (v: unknown) =>
  typeof v === "number" && Number.isFinite(v)
    ? v
    : typeof v === "string" && /^\d+$/.test(v)
      ? Number(v)
      : undefined;
function rows(value?: CollectedData): Record<string, unknown>[] {
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
const quality = (value?: CollectedData) =>
  !value || value.quality === "notCollected"
    ? "尚未采集"
    : value.stale
      ? "观察已过期"
      : value.quality === "complete"
        ? "已采集"
        : value.quality;
export function homeFacts(controller?: SystemSettingsController) {
  const facts = controller?.summary.value;
  const network = facts?.network,
    storage = facts?.storage,
    power = facts?.power;
  const observations = controller?.state.observations;
  const nativeTime = (section: "network" | "power", label: string) => {
    const snapshot = observations?.get(section)?.snapshot;
    if (!snapshot) return "";
    const age =
      (controller?.state.now ?? Date.now()) - snapshot.collectedAtUnixMs;
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
  const systemDrive = [...(controller?.state.facts.values() ?? [])].find((f) =>
    f.source.domains.includes("hardware.os"),
  )?.value.fields["hardware.os.system_drive"];
  const volume = rows(storage).find(
    (r) =>
      r.system_volume === true ||
      (typeof systemDrive === "string" && r.DeviceID === systemDrive),
  );
  let total = number(volume?.size_bytes ?? volume?.Size),
    free = number(volume?.free_bytes ?? volume?.FreeSpace),
    shared = false;
  if (volume?.container_id) {
    const container = [...(controller?.state.facts.values() ?? [])]
      .filter((f) => f.source.domains.includes("hardware.containers"))
      .flatMap((f) => rows(f.value))
      .find((r) => r.id === volume.container_id);
    if (container) {
      total = number(container.size_bytes);
      free = number(container.free_bytes);
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
    network:
      typeof routeInterface === "string"
        ? routeInterface
        : network?.quality === "complete"
          ? `${interfaces.length} 个有地址的接口`
          : "未读取",
    networkDetail: `${typeof routeInterface === "string" ? `${nativeTime("network", "路由状态")} · ` : ""}${quality(network)}${network?.collectedAt ? ` · ${new Date(network.collectedAt * 1000).toLocaleTimeString()}` : ""}`,
    storageFree: validStorage ? (free! / 1000 ** 3).toFixed(1) : "—",
    storageDetail: validStorage
      ? `${shared ? "系统卷所属容器" : "系统卷"} · 共 ${(total! / 1000 ** 3).toFixed(1)} GB · ${quality(storage)}`
      : quality(storage),
    storageUsed: validStorage
      ? Math.round((1 - free! / total!) * 100)
      : undefined,
    battery:
      batteryPresent === false
        ? "无电池"
        : validPercent !== undefined
          ? `${Math.round(validPercent)}%`
          : "—",
    batteryDetail: `${typeof provider === "string" ? `${provider} · ${nativeTime("power", "供电状态")} · ` : ""}${quality(power)}${power?.collectedAt ? ` · ${new Date(power.collectedAt * 1000).toLocaleTimeString()}` : ""}`,
    batteryPercent: batteryPresent !== false ? validPercent : undefined,
  };
}
