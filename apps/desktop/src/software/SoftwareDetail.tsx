import { useState } from "react";
import { Alert, Button, Tabs, Tag } from "antd";
import { ArrowLeftOutlined } from "@ant-design/icons";
import { SoftwareCover, StatePanel } from "@rss-mdm-agent/ui";
import { useSnapshot } from "valtio";
import type { Snapshot } from "valtio/vanilla";
import type { Workspace } from "../state/workspace";
import type {
  InstalledSoftware,
  LocalSoftwareAction,
  SelfServiceDirectoryItem,
} from "../self-service/types";
import {
  localAction,
  localActionLabels,
  policyPresentation,
  scopeLabel,
} from "./presentation";
import { taskPhase } from "../task-center/controller";

export function SoftwareDetail({
  workspace,
  installed,
  policy,
  onBack,
  onPrepare,
}: {
  workspace: Workspace;
  installed?: Snapshot<InstalledSoftware>;
  policy?: Snapshot<SelfServiceDirectoryItem>;
  onBack(): void;
  onPrepare(action?: LocalSoftwareAction): void;
}) {
  const software = useSnapshot(workspace.software.state),
    [tab, setTab] = useState("overview"),
    presentation = policy ? policyPresentation(policy) : null,
    name = installed?.name ?? presentation?.name ?? "软件详情",
    update = installed ? localAction(installed, "update") : null,
    versions = [installed?.version, update?.version].filter(
      (value): value is string => !!value,
    ),
    related = installed
      ? [...software.operations.values()].filter(
          (value) =>
            JSON.stringify(value.prepared.choice.software) ===
            JSON.stringify(installed.reference),
        )
      : [],
    source = installed
      ? `${installed.origin} · ${scopeLabel(installed.scope)}`
      : "当前主体可见的组织发布目录";
  const actions = installed ? (
    <>
      {(["launch", "uninstall"] as const).map((action) => {
        const capability = localAction(installed, action);
        return (
          <span key={action} title={capability.reason}>
            <Button
              danger={action === "uninstall"}
              disabled={!capability.available}
              onClick={() => onPrepare(action)}
            >
              {action === "launch" ? "打开" : "卸载"}
            </Button>
          </span>
        );
      })}
      {update?.available && (
        <Button type="primary" onClick={() => onPrepare("update")}>
          更新至 {update.version}
        </Button>
      )}
    </>
  ) : (
    <Button type="primary" onClick={() => onPrepare()}>
      查看并准备
    </Button>
  );
  return (
    <div className="rss-page rss-software-page">
      <nav className="rss-software-breadcrumb" aria-label="软件详情路径">
        <Button type="text" icon={<ArrowLeftOutlined />} onClick={onBack}>
          返回软件中心
        </Button>
        <span>/ {name}</span>
      </nav>
      <header className="rss-software-detail-hero">
        <span className="rss-detail-app-icon">{name.slice(0, 1)}</span>
        <div className="rss-grow">
          <h1>{name}</h1>
          <p>
            {installed?.publisher ?? presentation?.publisher ?? "发布方未提供"}{" "}
            · {presentation?.category ?? "本机安装项"}
          </p>
          <Tag>{installed ? "本机安装事实" : "组织发布项目"}</Tag>
        </div>
        <div className="rss-page-actions">{actions}</div>
      </header>
      <div className="rss-software-detail-layout">
        <section className="rss-software-detail-content">
          <SoftwareCover
            title={name}
            subtitle={installed ? "YOUR WORKSPACE" : presentation?.category}
            tone={presentation?.tone ?? "workspace"}
          />
          <Tabs
            activeKey={tab}
            onChange={setTab}
            items={[
              { key: "overview", label: "概览" },
              { key: "versions", label: "版本记录" },
              { key: "permissions", label: "权限与范围" },
              { key: "activity", label: "相关任务" },
            ]}
          />
          {tab === "overview" ? (
            <div className="rss-stack">
              <h2>关于 {name}</h2>
              <p>
                {presentation?.description ??
                  "来自本机已读取的安装清单。此记录用于查看、打开和管理当前安装项；不据名称推断组织发布关联。"}
              </p>
              <div className="rss-software-highlight-grid">
                <article>
                  <h3>{installed ? "真实安装记录" : "当前可见发布"}</h3>
                  <p>{source}</p>
                </article>
                <article>
                  <h3>核对后再执行</h3>
                  <p>准备会固定对象与操作，实际执行仍由原服务检查。</p>
                </article>
                <article>
                  <h3>原任务可追踪</h3>
                  <p>回执丢失时查询原请求，进度与效果核实分别查看。</p>
                </article>
              </div>
              <div className="rss-software-help">
                <h3>使用中遇到问题？</h3>
                <p>向电脑助手描述问题，发送前可检查当前页面上下文。</p>
                <Button
                  onClick={() => {
                    workspace.assistant.draft.set(
                      `我在使用 ${name} 时遇到了问题，请帮我排查。`,
                    );
                    workspace.navigation.select("assistant");
                  }}
                >
                  描述问题 →
                </Button>
              </div>
            </div>
          ) : tab === "versions" ? (
            <div className="rss-stack">
              <h2>版本记录</h2>
              {versions.length ? (
                [...new Set(versions)].map((version) => (
                  <article className="rss-release-entry" key={version}>
                    <h3>
                      {version}{" "}
                      <Tag>
                        {version === installed?.version
                          ? "当前安装"
                          : "固定更新候选"}
                      </Tag>
                    </h3>
                    <p>{source}</p>
                  </article>
                ))
              ) : (
                <StatePanel
                  kind="readonly"
                  title="发布目录未提供软件版本历史"
                  description="Policy 与资源版本是执行定义版本，不作为软件发行版本。"
                />
              )}
              <p className="rss-muted">
                下载大小、发行说明与完整历史尚未提供。
              </p>
            </div>
          ) : tab === "permissions" ? (
            <div className="rss-stack">
              <h2>本次可用范围</h2>
              <dl className="rss-account-facts">
                <dt>来源</dt>
                <dd>{source}</dd>
                <dt>管理方式</dt>
                <dd>
                  {installed
                    ? localAction(installed, "uninstall").reason ||
                      "服务提供本地卸载准备"
                    : "组织发布，执行时核对授权"}
                </dd>
                <dt>作用主体</dt>
                <dd>
                  {installed
                    ? scopeLabel(installed.scope)
                    : policy?.subject.kind === "device"
                      ? "设备"
                      : "当前企业用户"}
                </dd>
                <dt>执行身份</dt>
                <dd>
                  {policy?.task.kind === "software"
                    ? policy.task.runAs
                    : "由原服务在准备中确定"}
                </dd>
              </dl>
              {installed &&
                installed.actions.map((value) => (
                  <div className="rss-setting-row" key={value.action}>
                    <span>{localActionLabels[value.action]}</span>
                    <span>
                      {value.available
                        ? "可尝试准备"
                        : localAction(installed, value.action).reason}
                    </span>
                  </div>
                ))}
              <Alert
                type="info"
                title="页面可见不代表执行许可；系统隐私授权清单尚未提供。"
              />
            </div>
          ) : (
            <div className="rss-stack">
              <h2>与 {name} 相关的原任务</h2>
              {software.historyError && (
                <Alert type="warning" title={software.historyError} />
              )}
              {related.map((value) => (
                <Button
                  className="rss-related-task"
                  key={value.prepared.request}
                  onClick={() => {
                    workspace.navigation.select("tasks", "history");
                    void workspace.tasks.openTask({
                      kind: "request",
                      request: value.prepared.request,
                    });
                  }}
                >
                  {value.prepared.choice.action} ·{" "}
                  {taskPhase[value.execution.phase]}
                  <small>{value.prepared.request}</small>
                </Button>
              ))}
              {!related.length && (
                <StatePanel
                  kind="empty"
                  title="当前读取范围内没有精确关联的任务"
                  description="只按原安装引用关联，不按软件名称匹配。"
                />
              )}
              {!software.historyComplete && (
                <Button
                  disabled={software.historyBusy}
                  onClick={() =>
                    void workspace.software.history(!!software.historyNext)
                  }
                >
                  读取原操作记录
                </Button>
              )}
            </div>
          )}
        </section>
        <aside className="rss-software-facts">
          <h3>软件信息</h3>
          <dl className="rss-account-facts">
            <dt>发布方</dt>
            <dd>
              {installed?.publisher ?? presentation?.publisher ?? "未提供"}
            </dd>
            <dt>类别</dt>
            <dd>{presentation?.category ?? "安装清单未提供"}</dd>
            <dt>当前版本</dt>
            <dd>{installed?.version ?? "尚未提供安装关联"}</dd>
            <dt>更新候选</dt>
            <dd>{update?.version ?? "未提供"}</dd>
            <dt>来源 / 范围</dt>
            <dd>{source}</dd>
            <dt>原生标识</dt>
            <dd className="rss-reference">{installed?.nativeId ?? "不适用"}</dd>
            <dt>下载大小</dt>
            <dd>未提供</dd>
          </dl>
          <Button onClick={() => onPrepare()}>快速查看与准备</Button>
          <p className="rss-muted">
            独立本机事实与发布目录分别呈现；不会推断“已安装”或默认选择 latest。
          </p>
        </aside>
      </div>
    </div>
  );
}
