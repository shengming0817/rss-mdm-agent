import { useEffect, useState } from "react";
import { Alert, Button, Input, Tag } from "antd";
import {
  AppstoreOutlined,
  DatabaseOutlined,
  DesktopOutlined,
  FileTextOutlined,
  RightOutlined,
  SafetyOutlined,
  ThunderboltOutlined,
  ToolOutlined,
  WifiOutlined,
} from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { deepClone } from "valtio/vanilla/utils";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type { SelfServiceDirectoryItem } from "../self-service/types";
import { policyPresentation } from "../software/presentation";
import { NetworkDrawer } from "./NetworkDrawer";
import "./tools.css";

export function ToolsPage({
  workspace,
  section,
  active,
}: {
  workspace: Workspace;
  section: string;
  active: boolean;
}) {
  const directory = useSnapshot(workspace.directory.state),
    nav = useSnapshot(workspace.navigation.state),
    software = useSnapshot(workspace.software.state),
    [query, setQuery] = useState(""),
    [network, setNetwork] = useState(false),
    [collection, setCollection] = useState(false),
    [message, setMessage] = useState("");
  useEffect(() => {
    if (active && !workspace.directory.state.readAt)
      void workspace.directory.load();
  }, [workspace, active]);
  useEffect(() => {
    if (active && nav.preparation?.input.kind === "network") setNetwork(true);
  }, [active, nav.preparation]);
  useEffect(() => {
    const target = nav.target;
    if (!active || target?.kind !== "resource" || target.page !== "tools")
      return;
    const item = directory.items.find(
      (value) =>
        `policy:${value.policyId}` === target.id &&
        JSON.stringify([
          value.policyId,
          value.policyVersion,
          value.policyRevision,
          value.subject,
        ]) === target.revision,
    );
    if (item && workspace.policies.clear()) {
      void workspace.policies.select(
        deepClone(item) as SelfServiceDirectoryItem,
      );
      workspace.navigation.showPolicy();
    }
  }, [active, nav.target]);
  const tools = [
    {
      id: "network-check",
      title: "网络连通性检查",
      description: "检查指定主机的 DNS 解析与 TCP 端口连通性，保留原检测记录。",
      Icon: WifiOutlined,
      group: "network",
      label: "开始检查",
      action: () => setNetwork(true),
    },
    {
      id: "storage",
      title: "查看存储空间",
      description: "查看本机空间观察与来源时间，了解系统卷容量。",
      Icon: DatabaseOutlined,
      group: "maintenance",
      label: "查看存储",
      action: () => workspace.navigation.select("system", "storage"),
    },
    {
      id: "power",
      title: "电池与电源",
      description: "查看电池状态、供电方式与获准的睡眠偏好。",
      Icon: ThunderboltOutlined,
      group: "maintenance",
      label: "查看电源",
      action: () => workspace.navigation.select("system", "power"),
    },
    {
      id: "security",
      title: "隐私与安全性",
      description: "查看本机保护观察，前往系统管理相应权限。",
      Icon: SafetyOutlined,
      group: "security",
      label: "查看保护",
      action: () => workspace.navigation.select("system", "security"),
    },
    {
      id: "diagnostics",
      title: "生成脱敏诊断",
      description: "先预览经本机服务脱敏的内容，再选择是否保存。",
      Icon: ToolOutlined,
      group: "security",
      label: "打开诊断",
      action: () => workspace.navigation.select("settings", "diagnostics"),
    },
    {
      id: "tasks",
      title: "查看最近任务",
      description: "读取原操作的进度、执行结果与效果核实，避免重复发起。",
      Icon: FileTextOutlined,
      group: "maintenance",
      label: "查看任务",
      action: () => workspace.navigation.select("tasks", "history"),
    },
    {
      id: "software-collection",
      title: "更新软件清单",
      description: "从已有采集来源提交刷新，按原请求核实采集结果。",
      Icon: AppstoreOutlined,
      group: "maintenance",
      label: "核对采集来源",
      action: () => {
        setCollection(true);
        if (!workspace.software.state.sources.length)
          void workspace.software.discover();
      },
    },
    {
      id: "host",
      title: "运行状态与服务",
      description: "查看 AI Host 与系统服务状态，按需受控重启 AI Host。",
      Icon: DesktopOutlined,
      group: "security",
      label: "查看运行状态",
      action: () => workspace.navigation.select("settings", "diagnostics"),
    },
  ];
  const visible = tools.filter(
      (tool) =>
        (section === "all" || section === tool.group) &&
        `${tool.title}${tool.description}`.includes(query),
    ),
    published = directory.items.filter(
      (item) =>
        item.task.kind === "script" &&
        `${policyPresentation(item).name}${policyPresentation(item).description}`.includes(
          query,
        ),
    );
  return (
    <div className="rss-page rss-tools-page">
      <p className="rss-eyebrow">DEVICE UTILITIES</p>
      <PageHeader
        title="工具中心"
        description="从查看状态开始，让设备上的小问题更容易处理。"
        actions={
          <Button onClick={() => void workspace.directory.load()}>
            重读发布目录
          </Button>
        }
      />
      <Input.Search
        aria-label="搜索设备工具"
        placeholder="搜索工具或组织发布项目"
        allowClear
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <div className="rss-tool-grid">
        {visible.map(({ id, title, description, Icon, label, action }) => (
          <article className="rss-tool-card" key={id}>
            <span className="rss-tool-glyph">
              <Icon />
            </span>
            <h2>{title}</h2>
            <p>{description}</p>
            <Button onClick={action}>
              {label} <RightOutlined />
            </Button>
          </article>
        ))}
      </div>
      {!visible.length && (
        <StatePanel kind="empty" title="没有匹配的设备工具" />
      )}
      {section === "all" && (
        <section className="rss-published-tools">
          <div className="rss-software-section-heading">
            <div>
              <h2>组织发布工具</h2>
              <p>当前主体可见的脚本项目；参数与执行许可由原服务核对。</p>
            </div>
            <Tag>{directory.complete ? "目录已读完" : "目录覆盖未完整"}</Tag>
          </div>
          {directory.error && <Alert type="warning" title={directory.error} />}
          {message && <Alert type="warning" title={message} />}
          {directory.loading && !published.length ? (
            <StatePanel kind="loading" title="正在读取组织工具" />
          ) : published.length ? (
            <div className="rss-tool-grid">
              {published.map((item) => {
                const view = policyPresentation(item);
                return (
                  <article className="rss-tool-card" key={item.policyId}>
                    <span className="rss-tool-glyph">
                      <ToolOutlined />
                    </span>
                    <h2>{view.name}</h2>
                    <p>{view.description}</p>
                    <small>
                      组织发布 ·{" "}
                      {item.task.kind === "script" ? item.task.runAs : ""} ·
                      风险 {item.selfService.riskLevel}
                    </small>
                    <Button
                      onClick={() => {
                        if (
                          workspace.policies.state.item?.policyId !==
                          item.policyId
                        ) {
                          if (!workspace.policies.clear()) {
                            setMessage("原提交结果尚未确认，请先查询原请求。");
                            workspace.navigation.showPolicy();
                            return;
                          }
                          void workspace.policies.select(
                            deepClone(item) as SelfServiceDirectoryItem,
                          );
                        }
                        workspace.navigation.showPolicy();
                      }}
                    >
                      查看并准备 <RightOutlined />
                    </Button>
                  </article>
                );
              })}
            </div>
          ) : (
            <StatePanel
              kind={directory.error ? "error" : "empty"}
              title="当前读取范围没有可见组织工具"
              description="不会用原型工具或本地任意命令替代组织发布。"
              onRead={() => void workspace.directory.load()}
            />
          )}
          {directory.next && (
            <Button onClick={() => void workspace.directory.load(true)}>
              读取更多组织项目
            </Button>
          )}
        </section>
      )}
      {collection && (
        <SectionCard
          title="核对软件采集来源"
          extra={
            <Button type="text" onClick={() => setCollection(false)}>
              关闭
            </Button>
          }
        >
          <div className="rss-stack">
            {software.loading && (
              <StatePanel kind="loading" title="正在读取采集来源" />
            )}
            {software.error && <Alert type="warning" title={software.error} />}
            {software.sources.map((source) => (
              <div
                className="rss-setting-row"
                key={JSON.stringify(source.descriptor.reference)}
              >
                <span>
                  {source.descriptor.domains.join(" · ")}
                  <small>
                    {source.descriptor.available
                      ? "来源当前可用，提交时仍核对授权"
                      : "来源当前不可用"}
                  </small>
                </span>
                <Button
                  disabled={!source.descriptor.available || source.busy}
                  onClick={() => {
                    const original = workspace.software.state.sources.find(
                      (value) =>
                        JSON.stringify(value.descriptor.reference) ===
                        JSON.stringify(source.descriptor.reference),
                    );
                    if (original)
                      void workspace.software.refreshSource(original);
                  }}
                >
                  采集此来源
                </Button>
              </div>
            ))}
            {!software.loading && !software.sources.length && (
              <StatePanel kind="empty" title="尚无可用的软件采集来源" />
            )}
            {[...software.queries.values()].map((value) => (
              <Alert
                key={value.request}
                type="info"
                title={`原采集请求 · ${value.phase}`}
                description={
                  <Button
                    onClick={() =>
                      void workspace.software.queryRefresh(value.request)
                    }
                  >
                    查询原采集
                  </Button>
                }
              />
            ))}
          </div>
        </SectionCard>
      )}
      <NetworkDrawer
        workspace={workspace}
        open={active && network}
        onClose={() => setNetwork(false)}
      />
    </div>
  );
}
