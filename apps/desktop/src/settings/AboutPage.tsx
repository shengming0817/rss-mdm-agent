import { Tag } from "antd";
import { PageHeader, SectionCard } from "@rss-mdm-agent/ui";
import { useEffect, useState } from "react";
import type { Workspace } from "../state/workspace";
export function AboutPage({
  workspace,
  active,
}: {
  workspace: Workspace;
  active: boolean;
}) {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    if (active)
      void workspace
        .appVersion?.()
        .then((value) => {
          if (current) setVersion(value);
        })
        .catch(() => {
          if (current) setVersion(null);
        });
    return () => {
      current = false;
    };
  }, [workspace, active]);
  return (
    <div className="rss-page rss-preferences-page">
      <p className="rss-eyebrow">PREFERENCES</p>
      <PageHeader
        title="关于与隐私"
        description="桌面自助服务与人、AI 共用的受控执行客户端。"
      />
      <div className="rss-stack">
        <SectionCard>
          <div className="rss-about-brand">
            <i>r.</i>
            <div>
              <h2>RSS Agent</h2>
              <p>桌面版本 {version ?? "尚未读取"}</p>
            </div>
            <Tag>Windows / macOS</Tag>
          </div>
        </SectionCard>
        <SectionCard title="隐私与数据边界">
          <div className="rss-stack">
            <p>
              设备状态与操作由本机系统服务持有。组织目录、授权与设备管理由所属服务核对；页面可见不代表执行许可。
            </p>
            <p>
              AI
              会话使用你选择的连接，发送的内容会交给对应提供方。发送前可以核对页面上下文与输入；切换账号会清理当前账号的页面草稿与上下文。
            </p>
            <p>
              已保存的连接秘密不会回显。个人 Passkey
              属于当前系统用户，密码和用户验证只在受保护的原生对话框中进行。
            </p>
            <p>
              诊断由本机服务脱敏，先预览固定快照，再由你选择保存位置。诊断导出不包含连接密钥或完整会话。
            </p>
          </div>
        </SectionCard>
      </div>
    </div>
  );
}
