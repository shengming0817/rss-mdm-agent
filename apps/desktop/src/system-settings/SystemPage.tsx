import { useEffect, useState } from "react";
import { Alert, Button, Progress, Tag } from "antd";
import {
  BulbOutlined,
  DatabaseOutlined,
  DesktopOutlined,
  InfoCircleOutlined,
  RightOutlined,
  SafetyOutlined,
  SoundOutlined,
  ThunderboltOutlined,
  WifiOutlined,
} from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type { SettingsSection } from "../self-service/types";
import { systemEntries } from "./catalog";
import { homeFacts } from "./facts";
import { display, labels } from "./presentation";
import { SettingsEditor } from "./SettingsEditor";
import { usePlatform } from "../state/react";
import "./system.css";

const icons = {
  network: WifiOutlined,
  display: BulbOutlined,
  sound: SoundOutlined,
  power: ThunderboltOutlined,
  storage: DatabaseOutlined,
  security: SafetyOutlined,
  about: InfoCircleOutlined,
};
const effects: Record<string, string> = {
  pending: "等待原操作",
  applied: "已核实应用",
  preview: "正在预览",
  restored: "已核实恢复",
  conflict: "观察版本冲突",
  failed: "操作失败",
  unknown: "结果尚未确认",
  openRequested: "已请求打开系统设置",
  observed: "已读取观察",
  cancelled: "已取消",
};
export function SystemPage({
  workspace,
  section,
  active,
}: {
  workspace: Workspace;
  section: string;
  active: boolean;
}) {
  const [visited, setVisited] = useState<string[]>([section]);
  useEffect(() => {
    setVisited((values) =>
      values.includes(section) ? values : [...values, section],
    );
  }, [section]);
  return (
    <>
      {[...new Set([...visited, section])].map((id) => (
        <div key={id} hidden={id !== section}>
          <SystemSection
            workspace={workspace}
            section={id}
            active={active && id === section}
          />
        </div>
      ))}
    </>
  );
}
function SystemSection({
  workspace,
  section,
  active,
}: {
  workspace: Workspace;
  section: string;
  active: boolean;
}) {
  const state = useSnapshot(workspace.system.state),
    nav = useSnapshot(workspace.navigation.state),
    device = useSnapshot(workspace.device.state),
    platform = usePlatform(),
    entries = systemEntries(platform === "macos"),
    facts = homeFacts(workspace.system, state),
    selected = entries.find((entry) => entry.id === section),
    scope = (section === "overview" ? "about" : section) as SettingsSection,
    response = state.observations.get(scope),
    snapshot = response?.snapshot,
    pending = state.pending.get(scope),
    overviewSections = ["network", "storage", "power", "about"] as const,
    busy =
      section === "overview"
        ? overviewSections.some(
            (id) => state.busy.has(id) || state.reads.has(id),
          ) || device.loading
        : state.busy.has(scope) || state.reads.has(scope),
    relevant = workspace.system.relevant(scope, state),
    attributes = device.value?.attributes;
  const read = () => {
    if (section === "overview") {
      void workspace.device.refresh();
      for (const id of overviewSections) void workspace.system.load(id);
    } else void workspace.system.load(scope);
  };
  useEffect(() => {
    if (!active) return;
    read();
  }, [workspace, section, active]);
  const summaries: Record<string, string> = {
    network: `${facts.network} · ${facts.networkDetail}`,
    display: "显示器、分辨率与缩放",
    sound: "本机输入与输出设备",
    power: `${facts.battery} · ${facts.batteryDetail}`,
    storage: `${facts.storageFree} GB 可用 · ${facts.storageDetail}`,
    security: "设备保护与系统权限观察",
    about: "版本、硬件与本机身份",
  };
  const openTask = (request: string) => {
    workspace.navigation.select("tasks", "history");
    void workspace.tasks.openTask({ kind: "request", request });
  };
  return (
    <div className="rss-page rss-system-page">
      <p className="rss-eyebrow">THIS COMPUTER</p>
      <PageHeader
        title="系统设置"
        description="查看设备状态，按权限调整当前设备的设置。"
        actions={
          <Button disabled={busy} onClick={read}>
            重读状态
          </Button>
        }
      />
      {section === "overview" ? (
        <>
          {busy && <StatePanel kind="loading" title="正在读取设备与系统观察" />}
          {device.error && <Alert type="warning" title={device.error} />}
          {overviewSections
            .filter((id) => state.errors.has(id))
            .map((id) => (
              <Alert
                key={id}
                type="warning"
                title={`${entries.find((entry) => entry.id === id)?.title ?? id}：${state.errors.get(id)}`}
              />
            ))}
          {!workspace.system.available && (
            <StatePanel kind="readonly" title="当前装配未提供本机设置能力" />
          )}
          {!busy &&
            !device.value &&
            !state.facts.size &&
            !overviewSections.some(
              (id) => state.observations.get(id)?.snapshot?.targets.length,
            ) && (
              <StatePanel
                kind="empty"
                title="尚无可读设备观察"
                description="可重新读取各来源；未读取字段不会显示为零或关闭。"
              />
            )}
          <div className="rss-system-banner">
            <DesktopOutlined />
            <div>
              <h2>{attributes?.computerName ?? "本机设备"}</h2>
              <p>
                {attributes?.loginUser
                  ? `当前系统用户 · ${attributes.loginUser}`
                  : "系统用户尚未读取"}
              </p>
              <p>
                {device.readAt == null
                  ? "设备观察尚未取得"
                  : `本机概览 · ${new Date(device.readAt).toLocaleString()}`}
              </p>
            </div>
          </div>
          <div className="rss-system-cards">
            {entries.map((entry) => {
              const Icon = icons[entry.id as keyof typeof icons];
              return (
                <button
                  type="button"
                  key={entry.id}
                  className="rss-system-card"
                  onClick={() =>
                    workspace.navigation.select("system", entry.id)
                  }
                >
                  <span className={`rss-system-glyph ${entry.color}`}>
                    <Icon />
                  </span>
                  <span className="rss-grow">
                    <h3>{entry.title}</h3>
                    <p>{summaries[entry.id]}</p>
                  </span>
                  <RightOutlined />
                </button>
              );
            })}
          </div>
        </>
      ) : (
        <div className="rss-stack">
          {nav.preparation?.input.kind === "settings" && (
            <Alert
              type="info"
              title="已定位本机入口，请核对目标、权限与参数后手动操作。"
              description={
                <Button onClick={() => workspace.navigation.clearPreparation()}>
                  忽略入口参数
                </Button>
              }
            />
          )}
          <div className="rss-system-section-heading">
            <h2>{selected?.title ?? section}</h2>
            <Button
              disabled={!workspace.system.available || busy}
              onClick={() => void workspace.system.openNative(scope)}
            >
              打开系统设置
            </Button>
          </div>
          {state.errors.get(scope) && (
            <Alert type="warning" title={state.errors.get(scope)} />
          )}
          {response?.reason && (
            <Alert
              type={
                response.effect === "conflict" || response.effect === "failed"
                  ? "warning"
                  : "info"
              }
              title={response.reason}
            />
          )}
          {response && (
            <div className="rss-system-observation">
              <Tag>{effects[response.effect] ?? "状态待核实"}</Tag>
              <span>
                {snapshot
                  ? `${snapshot.source} · ${new Date(snapshot.collectedAtUnixMs).toLocaleString()}${snapshot.collectedAtUnixMs > state.now || state.now - snapshot.collectedAtUnixMs >= 30000 ? " · 观察待更新" : ""}`
                  : "原生观察尚未取得"}
              </span>
            </div>
          )}
          {!workspace.system.available && (
            <StatePanel
              kind="readonly"
              title="当前装配未提供本机设置能力"
              description="采集观察和原生设置分别读取，可继续查看已取得事实。"
            />
          )}
          {busy && !snapshot && (
            <StatePanel kind="loading" title="正在读取系统观察" />
          )}
          {section === "storage" && (
            <SectionCard title="本机存储">
              <div className="rss-storage-number">
                {facts.storageFree}
                <small> GB 可用</small>
              </div>
              <p className="rss-muted">{facts.storageDetail}</p>
              {facts.storageUsed != null && (
                <Progress percent={facts.storageUsed} showInfo={false} />
              )}
              <Alert
                type="info"
                title="空间数据只用于查看；此页面不会扫描或删除文件。"
              />
            </SectionCard>
          )}
          {snapshot?.targets.map((target) => (
            <SectionCard key={target.id} title={target.name}>
              <div>
                {target.fields.map((field) => (
                  <div className="rss-setting-row" key={field.key}>
                    <span>
                      <strong>{labels[field.key] ?? field.key}</strong>
                      {field.reason && <small>{field.reason}</small>}
                    </span>
                    <span className="rss-setting-value">
                      {field.quality === "complete" ||
                      field.quality === "partial"
                        ? display(field.value)
                        : "未取得可靠观察"}
                      {field.quality !== "complete" && (
                        <small>{field.quality}</small>
                      )}
                    </span>
                  </div>
                ))}
              </div>
              {target.reason && <p className="rss-muted">{target.reason}</p>}
              {target.fields.some((field) => field.writable) && (
                <SettingsEditor
                  key={`${scope}:${target.id}`}
                  owner={workspace.system}
                  target={target}
                  section={scope}
                  revision={snapshot.revision}
                  disabled={!response?.writeAllowed || busy || !!pending}
                  entry={
                    nav.preparation?.input.kind === "settings"
                      ? nav.preparation.input
                      : undefined
                  }
                  onEntryUsed={() => workspace.navigation.clearPreparation()}
                />
              )}
            </SectionCard>
          ))}
          {section === "power" && (
            <SectionCard title="屏幕、睡眠与返回电脑">
              <p className="rss-muted">
                关闭屏幕、系统睡眠与锁定电脑分别由系统管理。返回登录要求可在隐私与安全性查看。
              </p>
              <Button
                onClick={() =>
                  workspace.navigation.select("system", "security")
                }
              >
                查看隐私与安全性
              </Button>
            </SectionCard>
          )}
          {relevant.map((source) => {
            const fact = state.facts.get(source.reference.id);
            return (
              <SectionCard
                key={JSON.stringify(source.reference)}
                title={source.domains.join(" · ")}
                extra={
                  <Tag>
                    {fact?.value.stale
                      ? "观察过期"
                      : (fact?.value.quality ?? "未读取")}
                  </Tag>
                }
              >
                <p className="rss-system-source">
                  本机采集来源 ·{" "}
                  {fact?.value.collectedAt == null
                    ? "尚无观察时间"
                    : new Date(fact.value.collectedAt * 1000).toLocaleString()}
                </p>
                {fact ? (
                  Object.entries(fact.value.fields)
                    .slice(0, 20)
                    .map(([key, value]) => (
                      <div className="rss-setting-row" key={key}>
                        <span>{labels[key] ?? key.split(".").at(-1)}</span>
                        <span className="rss-setting-value">
                          {display(value)}
                        </span>
                      </div>
                    ))
                ) : (
                  <StatePanel kind="unknown" title="尚未读取此来源" />
                )}
                {fact && fact.value.rows.length > 0 && (
                  <p className="rss-muted">
                    已读取 {fact.value.rows.length} 条记录；完整性：
                    {fact.value.quality}。
                  </p>
                )}
              </SectionCard>
            );
          })}
          {!busy && !snapshot?.targets.length && !relevant.length && (
            <StatePanel
              kind={state.errors.has(scope) ? "error" : "empty"}
              title="当前范围没有可读观察"
              description="可以重读状态，或打开系统对应页面查看。"
              onRead={() => void workspace.system.load(scope)}
            />
          )}
          <p className="rss-muted">{selected?.note}</p>
          <div className="rss-settings-actions">
            <span className="rss-grow" />
            {pending && (
              <>
                <Button
                  disabled={busy}
                  onClick={() => void workspace.system.recover(scope)}
                >
                  查询原操作
                </Button>
                <Button onClick={() => openTask(pending.request)}>
                  查看原任务
                </Button>
              </>
            )}
            {response?.effect === "preview" && (
              <>
                <span>
                  保留期限：
                  {response.keepBeforeUnixMs == null
                    ? "未取得"
                    : new Date(response.keepBeforeUnixMs).toLocaleTimeString()}
                </span>
                <Button
                  type="primary"
                  disabled={
                    busy ||
                    response.keepBeforeUnixMs == null ||
                    state.now >= response.keepBeforeUnixMs
                  }
                  onClick={() => void workspace.system.keep(scope)}
                >
                  保留显示设置
                </Button>
                <Button
                  disabled={busy}
                  onClick={() => void workspace.system.cancel(scope)}
                >
                  请求结束预览
                </Button>
              </>
            )}
            <Button
              disabled={busy}
              onClick={() => void workspace.system.load(scope, true)}
            >
              采集最新状态
            </Button>
          </div>
          {[...state.queries.values()]
            .filter((query) =>
              relevant.some(
                (source) =>
                  JSON.stringify(source.reference) ===
                  JSON.stringify(query.query),
              ),
            )
            .map((query) => (
              <Alert
                key={query.request}
                type={query.phase === "unknown" ? "warning" : "info"}
                title={`原采集请求 · ${query.phase}`}
                description={
                  <Button
                    disabled={busy}
                    onClick={() =>
                      void workspace.system.recoverQuery(scope, query.request)
                    }
                  >
                    查询原采集
                  </Button>
                }
              />
            ))}
        </div>
      )}
    </div>
  );
}
