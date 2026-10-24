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
};
export function display(v: unknown): string {
  if (v === null || v === undefined) return "未知";
  if (typeof v === "boolean") return v ? "已启用" : "已关闭";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
