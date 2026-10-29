import type { Snapshot } from "valtio/vanilla";
import type { Workspace } from "../state/workspace";
import type { PageId } from "../state/navigation";
import type { PageResource } from "./page-navigation";
import { homeFacts } from "../system-settings/facts";
import { taskKey, taskTitle } from "../task-center/controller";
export function pageResources(
  workspace: Workspace,
  page: PageId,
  section: string,
  device: Snapshot<Workspace["device"]["state"]>,
  system: Snapshot<Workspace["system"]["state"]>,
  software: Snapshot<Workspace["software"]["state"]>,
  tasks: Snapshot<Workspace["tasks"]["state"]>,
  now = Date.now(),
): PageResource[] {
  const rows: PageResource[] = [];
  if (page === "home" && device.value && !device.error && device.readAt != null)
    rows.push({
      id: "device",
      title: device.value.attributes.computerName ?? "本机设备",
      description: `系统用户：${device.value.attributes.loginUser ?? "未读取"}；网络地址：${device.value.attributes.ipAddresses?.join("、") ?? "未读取"}`,
      source: "本机设备概览",
      collectedAt: device.readAt,
      validUntil: device.readAt + 30000,
      verified: device.readAt <= now,
    });
  if (page === "home" || page === "system") {
    const facts = homeFacts(workspace.system, system, now);
    for (const [id, title, text, evidence] of [
      [
        "network",
        "网络接口",
        facts.network + " · " + facts.networkDetail,
        facts.networkContext,
      ],
      [
        "storage",
        "存储空间",
        facts.storageFree + " GB · " + facts.storageDetail,
        facts.storageContext,
      ],
      [
        "power",
        "电池与电源",
        facts.battery + " · " + facts.batteryDetail,
        facts.powerContext,
      ],
    ] as const) {
      if (
        (page === "home" || section === "overview" || section === id) &&
        evidence.collectedAt != null &&
        !system.errors.has(id)
      )
        rows.push({
          id,
          title,
          description: text,
          source: "本机系统观察",
          collectedAt: evidence.collectedAt,
          validUntil: evidence.validUntil,
          verified: evidence.verified,
        });
    }
  }
  if (
    page === "home" ||
    (page === "software" && ["installed", "updates"].includes(section))
  ) {
    const inventory = workspace.software.summary(software);
    for (const item of inventory.items.slice(0, 12))
      rows.push({
        id: item.reference.installation,
        title: item.name,
        description: `已读取安装版本 ${item.version ?? "未知"}`,
        version: item.version ?? undefined,
        source: "本机安装清单",
        revision: JSON.stringify(item.reference),
        verified:
          inventory.complete &&
          !inventory.error &&
          inventory.collectedAt != null &&
          inventory.collectedAt * 1000 <= now,
        ...(inventory.collectedAt == null
          ? {}
          : {
              collectedAt: inventory.collectedAt * 1000,
              validUntil: inventory.collectedAt * 1000 + 300000,
            }),
      });
  }
  if (page === "home" || page === "tasks")
    for (const item of workspace.tasks.recentItems(tasks).slice(0, 3))
      rows.push({
        id: taskKey(item.reference),
        title: taskTitle(item),
        description: `原任务阶段：${item.stage}`,
        status: item.stage,
        source: "当前主体任务记录",
        revision: JSON.stringify(item.revision),
        collectedAt: tasks.recentAsOf,
        validUntil: tasks.recentAsOf + 30000,
        verified:
          tasks.recentLoaded &&
          !tasks.recentError &&
          tasks.recentAsOf > 0 &&
          tasks.recentAsOf <= now,
      });
  return rows;
}
