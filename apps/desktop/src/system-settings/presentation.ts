export const labels: Record<string, string> = {
  http: "HTTP 代理",
  https: "HTTPS 代理",
  socks: "SOCKS 代理",
  bypass: "不使用代理的地址",
  pacEnabled: "自动配置脚本",
  wpadEnabled: "自动发现代理",
  enabled: "网络服务启用",
  mode: "当前模式",
  width: "逻辑宽度",
  height: "逻辑高度",
  pixelWidth: "像素宽度",
  pixelHeight: "像素高度",
  origin: "桌面位置",
  main: "主显示器",
  input: "声音输入",
  output: "声音输出",
  volume: "输出音量",
  displayOffMinutes: "关闭屏幕（分钟）",
  sleepMinutes: "系统睡眠（分钟）",
  requireLogin: "返回时要求登录",
  facts: "本机状态",
  interface: "网络接口",
  addresses: "网络地址",
  batteryPresent: "存在电池",
  source: "供电来源",
  name: "名称",
  version: "版本",
  model: "型号",
  manufacturer: "制造商",
  serial_number: "序列号",
  size_bytes: "容量",
  free_bytes: "可用空间",
  system_volume: "系统卷",
  cpu_count: "处理器数量",
  memory_bytes: "内存容量",
  charge_percent: "电量百分比",
  percentage: "百分比",
};
const qualityLabels: Record<string, string> = {
  complete: "完整观察",
  partial: "部分观察",
  truncated: "已截断",
  notCollected: "尚未采集",
  missing: "缺少记录",
  failed: "读取失败",
  unsupported: "当前不支持",
  denied: "无权读取",
  expired: "观察过期",
  definitionUnavailable: "来源定义不可用",
  unknown: "结果未知",
};
const sourceLabels: Record<string, string> = {
  "hardware.network": "网络接口",
  "hardware.volumes": "本机卷",
  "hardware.disks": "物理磁盘",
  "hardware.containers": "存储容器",
  "hardware.battery": "电池观察",
  "hardware.os": "操作系统",
  "hardware.cpu": "处理器",
  "hardware.memory": "内存",
  "hardware.system": "设备硬件",
  "hardware.identity": "本机身份",
};
export const qualityLabel = (value: string) =>
  qualityLabels[value] ?? "状态未知";
export const sourceLabel = (domains: readonly string[]) =>
  [
    ...new Set(
      domains.map(
        (domain) =>
          sourceLabels[domain] ??
          (domain.startsWith("security.") ? "设备安全观察" : "本机采集观察"),
      ),
    ),
  ].join(" · ");
export function fieldLabel(key: string) {
  return labels[key] ?? labels[key.split(".").at(-1) ?? ""];
}
export function fieldValue(key: string, value: unknown) {
  if (
    ["http", "https", "socks"].includes(key) &&
    value &&
    typeof value === "object" &&
    "enabled" in value
  ) {
    if (value.enabled === false) return "已关闭";
    if (value.enabled === true && "host" in value && "port" in value)
      return `${String(value.host)}:${String(value.port)}`;
    return "代理状态未确认";
  }
  if (key.endsWith("_bytes") && value != null) {
    const bytes = Number(value);
    if (Number.isFinite(bytes) && bytes >= 0)
      return `${(bytes / 1000 ** 3).toFixed(1)} GB`;
  }
  return display(value);
}
export function display(v: unknown): string {
  if (v === null || v === undefined) return "未知";
  if (typeof v === "boolean") return v ? "是" : "否";
  if (Array.isArray(v) && v.every((value) => typeof value === "string"))
    return v.join("、") || "已读取，结果为空";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
