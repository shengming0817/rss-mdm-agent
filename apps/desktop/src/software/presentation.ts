import type { Snapshot } from "valtio/vanilla";
import type { CoverTone } from "@rss-mdm-agent/ui";
import type {
  DataScope,
  InstalledSoftware,
  SelfServiceDirectoryItem,
  SoftwareBlock,
} from "../self-service/types";

export const categories = [
  "效率办公",
  "创意设计",
  "网络工具",
  "开发工具",
  "系统工具",
] as const;
export const categoryTones: Record<string, CoverTone> = {
  效率办公: "workspace",
  创意设计: "design",
  网络工具: "browser",
  开发工具: "notes",
  系统工具: "system",
};
export function boundedText(value: unknown, max = 300): string {
  return typeof value === "string" ? value.slice(0, max) : "";
}
export function policyPresentation(item: Snapshot<SelfServiceDirectoryItem>) {
  const presentation = item.selfService.presentation,
    category = boundedText(presentation.category, 40);
  return {
    name: boundedText(presentation.displayName, 120) || item.policyId,
    description:
      boundedText(presentation.description, 1000) || "发布方尚未提供项目说明。",
    category: (categories as readonly string[]).includes(category)
      ? category
      : "未分类",
    publisher: boundedText(presentation.publisher, 120) || "发布方未提供",
    tone: categoryTones[category] ?? "workspace",
  };
}
export const scopeLabel = (scope: Snapshot<DataScope>) =>
  scope.kind === "device" ? "设备范围" : `用户范围 · ${scope.subject}`;
export const blockReason: Record<SoftwareBlock, string> = {
  protected: "由组织保护，不允许本地卸载。",
  ruleDisabled: "管理员已停用此操作。",
  identityUnknown: "当前安装身份未确认。",
  updateUnknown: "没有已核实的固定更新候选。",
  noGui: "此安装项没有可打开的图形应用。",
  unsupported: "当前安装来源不支持此操作。",
};
export function localAction(
  item: Snapshot<InstalledSoftware>,
  action: "launch" | "update" | "uninstall" | "detect",
) {
  const choice = item.actions.find((value) => value.action === action);
  return {
    available: !!choice?.available,
    reason: choice?.reason
      ? blockReason[choice.reason]
      : choice?.available
        ? ""
        : "此来源未提供此操作。",
    version: choice?.targetVersion,
  };
}

export const localActionLabels = {
  launch: "打开",
  uninstall: "卸载",
  update: "更新",
  detect: "检测",
} as const;
