import {
  AppstoreOutlined,
  ClockCircleOutlined,
  HomeOutlined,
  RobotOutlined,
  SettingOutlined,
  ToolOutlined,
  PlusOutlined,
  HistoryOutlined,
  CompassOutlined,
  DownloadOutlined,
  CheckCircleOutlined,
  FolderOutlined,
  WifiOutlined,
  SafetyOutlined,
  InfoCircleOutlined,
  UserOutlined,
  LockOutlined,
  BulbOutlined,
} from "@ant-design/icons";
import { modules, sections, type PageId } from "../state/navigation";
import type { ComponentType } from "react";
const moduleIcons = [
  HomeOutlined,
  RobotOutlined,
  AppstoreOutlined,
  ToolOutlined,
  SettingOutlined,
  ClockCircleOutlined,
];
export const moduleEntries = modules.map(([id, title], index) => ({
  id,
  title,
  Icon: moduleIcons[index],
}));
export const pageTitle = (page: PageId) =>
  page === "settings"
    ? "账号设置"
    : (moduleEntries.find((entry) => entry.id === page)?.title ?? "RSS Agent");
const descriptions: Record<
  string,
  { title: string; Icon: ComponentType; group?: string }
> = {
  overview: { title: "概览", Icon: InfoCircleOutlined },
  new: { title: "新会话", Icon: PlusOutlined },
  history: { title: "历史记录", Icon: HistoryOutlined },
  discover: { title: "发现", Icon: CompassOutlined, group: "软件中心" },
  installed: { title: "已安装", Icon: CheckCircleOutlined },
  updates: { title: "更新", Icon: DownloadOutlined },
  all: { title: "全部工具", Icon: ToolOutlined },
  network: { title: "连接与网络", Icon: WifiOutlined },
  maintenance: { title: "系统维护", Icon: SettingOutlined },
  security: { title: "隐私与安全性", Icon: SafetyOutlined },
  display: { title: "显示器", Icon: BulbOutlined },
  sound: { title: "声音", Icon: InfoCircleOutlined },
  power: { title: "电池与电源", Icon: InfoCircleOutlined },
  storage: { title: "存储空间", Icon: FolderOutlined },
  about: { title: "关于这台电脑", Icon: InfoCircleOutlined },
  active: { title: "正在进行", Icon: ClockCircleOutlined },
  management: { title: "管理活动", Icon: SafetyOutlined },
  account: { title: "账号与组织", Icon: UserOutlined },
  registration: { title: "设备注册与 MDM", Icon: HomeOutlined },
  connection: { title: "AI 与连接", Icon: RobotOutlined },
  "personal-vault": { title: "个人 Passkey", Icon: LockOutlined },
  diagnostics: { title: "运行与诊断", Icon: ToolOutlined },
  appearance: { title: "外观与体验", Icon: BulbOutlined },
};
export function secondaryEntries(page: PageId, platform: "macos" | "windows") {
  if (page === "home") return [];
  return sections[page].map((id) => {
    if (id.startsWith("category:"))
      return {
        id,
        title: id.slice(9),
        Icon: FolderOutlined,
        group: id === "category:效率办公" ? "按类别浏览" : undefined,
      };
    const description = descriptions[id];
    const title =
      page === "assistant" && id === "history"
        ? "历史会话"
        : page === "settings" && id === "about"
          ? "关于与隐私"
          : page === "system" && id === "network"
            ? platform === "windows"
              ? "网络和 Internet"
              : "网络"
            : page === "tools" && id === "security"
              ? "安全与支持"
              : description.title;
    return { id, ...description, title };
  });
}
