import { useEffect } from "react";
import { Alert, Button, Input, Tag } from "antd";
import {
  AppstoreOutlined,
  ArrowRightOutlined,
  ArrowUpOutlined,
  ClockCircleOutlined,
  DatabaseOutlined,
  DesktopOutlined,
  WifiOutlined,
  ThunderboltOutlined,
  CheckOutlined,
} from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { homeFacts } from "../system-settings/facts";
import { taskKey, taskPhase, taskTitle } from "../task-center/controller";
import { createRegistration } from "../account/registration";
import "./home.css";
export function HomePage({
  workspace,
  registration,
}: {
  workspace: Workspace;
  registration: ReturnType<typeof createRegistration> | null;
}) {
  const device = useSnapshot(workspace.device.state),
    system = useSnapshot(workspace.system.state),
    software = useSnapshot(workspace.software.state),
    assistant = useSnapshot(workspace.assistant.state),
    tasks = useSnapshot(workspace.tasks.state);
  const facts = homeFacts(workspace.system, system),
    inventory = workspace.software.summary(software),
    recent = workspace.tasks.recentItems(tasks).slice(0, 3),
    attributes = device.value?.attributes;
  const read = () => {
    void workspace.device.refresh();
    void workspace.software.discover();
    void workspace.system.load("network", false);
    void workspace.system.load("storage", false);
    void workspace.system.load("power", false);
  };
  useEffect(() => {
    read();
  }, [workspace]);
  const hour = new Date(system.now).getHours(),
    greeting = hour < 12 ? "早上好" : hour < 18 ? "下午好" : "晚上好";
  const ask = (text?: string) => {
    if (text) workspace.assistant.draft.set(text);
    workspace.navigation.select("assistant");
  };
  return (
    <div className="rss-page rss-home-page">
      <header className="rss-home-heading">
        <div>
          <p className="rss-eyebrow">YOUR EVERYDAY COMPANION</p>
          <h1>
            {greeting}
            {workspace.identity.identity.mode === "user_device"
              ? `，${workspace.identity.user.displayName}`
              : ""}
            。
          </h1>
          <p className="rss-muted">你的设备、应用和工作进度，都在这里。</p>
        </div>
        {registration && <RegistrationBadge owner={registration} />}
      </header>
      {device.error && (
        <div className="rss-home-notice">
          <StatePanel
            kind="error"
            title={device.error}
            onRead={() => void workspace.device.refresh()}
          />
        </div>
      )}
      <div className="rss-home-hero-grid">
        <section className="rss-device-hero">
          <span className="rss-eyebrow">MY WORKSPACE / 我的电脑</span>
          <h2>
            让工作顺畅，
            <br />
            让设备简单。
          </h2>
          <div className="rss-device-description">
            <p>
              {attributes?.computerName ||
                (device.loading ? "正在读取设备名称" : "设备名称未读取")}
            </p>
            <p>当前系统用户：{attributes?.loginUser ?? "尚未读取"}</p>
          </div>
          <Button
            onClick={() => workspace.navigation.select("system", "about")}
          >
            了解这台电脑 <ArrowRightOutlined />
          </Button>
          <div className="rss-home-laptop" aria-hidden="true">
            <div className="rss-home-screen">
              <div className="rss-home-wallpaper">
                <small>YOUR WORKSPACE</small>
                <strong>
                  {new Date(system.now).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </strong>
                <div className="rss-home-dock">
                  <i>W</i>
                  <i>F</i>
                  <i>✧</i>
                  <i>r.</i>
                </div>
              </div>
            </div>
            <div className="rss-home-laptop-base" />
          </div>
        </section>
        <section className="rss-assistant-hero">
          <span className="rss-home-ai-symbol" aria-hidden="true">
            ✧
          </span>
          <h2>
            有问题，有想法，
            <br />
            随时说说。
          </h2>
          <p>
            从寻找应用到排查问题，
            <br />
            电脑助手陪你把事情做好。
          </p>
          <div className="rss-home-prompt">
            <Input
              aria-label="主页助手草稿"
              placeholder="例如：为什么我的网络有点慢？"
              value={workspace.assistant.draft.get(assistant)}
              onChange={(event) =>
                workspace.assistant.draft.set(event.target.value)
              }
              onPressEnter={(event) => {
                if (!event.nativeEvent.isComposing && event.keyCode !== 229)
                  ask();
              }}
            />
            <Button
              type="primary"
              aria-label="在 AI 助手中继续"
              icon={<ArrowUpOutlined />}
              onClick={() => ask()}
            />
          </div>
          <div className="rss-home-suggestions">
            <button type="button" onClick={() => ask("帮我检查当前网络状态")}>
              检查网络 ↗
            </button>
            <button type="button" onClick={() => ask("帮我找个适合工作的应用")}>
              找个应用 ↗
            </button>
          </div>
        </section>
      </div>
      <div className="rss-home-stats">
        <button
          type="button"
          className="rss-home-stat"
          onClick={() => workspace.navigation.select("system", "network")}
        >
          <span className="rss-stat-label">
            <WifiOutlined /> 网络接口
          </span>
          <strong>{facts.network}</strong>
          <small>{system.errors.get("network") || facts.networkDetail}</small>
          <span className="rss-home-meter neutral" aria-hidden="true">
            <i style={{ width: facts.network !== "未读取" ? "100%" : "0%" }} />
          </span>
        </button>
        <button
          type="button"
          className="rss-home-stat"
          onClick={() => workspace.navigation.select("system", "storage")}
        >
          <span className="rss-stat-label">
            <DatabaseOutlined /> 存储空间
          </span>
          <strong>
            {facts.storageFree}
            <em>GB 可用</em>
          </strong>
          <small>{system.errors.get("storage") || facts.storageDetail}</small>
          {facts.storageUsed !== undefined && (
            <span
              className="rss-home-meter purple"
              aria-label={`存储已使用 ${facts.storageUsed}%`}
            >
              <i style={{ width: `${facts.storageUsed}%` }} />
            </span>
          )}
        </button>
        <button
          type="button"
          className="rss-home-stat"
          onClick={() => workspace.navigation.select("system", "power")}
        >
          <span className="rss-stat-label">
            <ThunderboltOutlined /> 电池与电源
          </span>
          <strong>{facts.battery}</strong>
          <small>{system.errors.get("power") || facts.batteryDetail}</small>
          {facts.batteryPercent !== undefined && (
            <span
              className="rss-home-meter"
              aria-label={`电量 ${facts.batteryPercent}%`}
            >
              <i style={{ width: `${facts.batteryPercent}%` }} />
            </span>
          )}
        </button>
        <button
          type="button"
          className="rss-home-stat"
          onClick={() => workspace.navigation.select("software", "installed")}
        >
          <span className="rss-stat-label">
            <AppstoreOutlined /> 我的应用
          </span>
          <strong>
            {inventory.read ? inventory.installed : "—"}
            <em>{inventory.complete ? "个已安装" : "个已读取"}</em>
          </strong>
          <small>
            {inventory.error ||
              (inventory.loading
                ? "正在读取本机安装清单"
                : inventory.read
                  ? inventory.complete
                    ? `${inventory.updates} 个可用本地更新候选`
                    : "已读取部分清单，覆盖尚未完整"
                  : "尚未读取安装清单")}
          </small>
        </button>
      </div>
      <div className="rss-home-lower">
        <section>
          <div className="rss-home-section-heading">
            <h3>常用应用</h3>
            <button
              type="button"
              onClick={() =>
                workspace.navigation.select("software", "installed")
              }
            >
              查看全部 <ArrowRightOutlined />
            </button>
          </div>
          <div className="rss-home-apps">
            {inventory.loading && !inventory.read ? (
              <StatePanel kind="loading" title="正在读取本机应用" />
            ) : inventory.items.length ? (
              inventory.items.slice(0, 4).map((item) => (
                <button
                  type="button"
                  className="rss-home-app"
                  key={item.reference.installation}
                  onClick={() =>
                    workspace.navigation.select("software", "installed")
                  }
                  aria-label={`查看 ${item.name}`}
                >
                  <span className="rss-home-app-icon">
                    {item.name.slice(0, 1)}
                  </span>
                  <span>{item.name}</span>
                </button>
              ))
            ) : (
              <StatePanel
                kind={inventory.error ? "error" : "empty"}
                title={
                  inventory.error ? "应用清单尚未读到" : "没有已读取的应用"
                }
                description={
                  inventory.error || "可从软件中心查看本机安装清单。"
                }
                onRead={
                  inventory.error
                    ? () => void workspace.software.discover()
                    : undefined
                }
              />
            )}
          </div>
          <p className="rss-home-source">
            来自本机安装清单；尚无使用频次记录。
          </p>
        </section>
        <section>
          <div className="rss-home-section-heading">
            <h3>最近动态</h3>
            <button
              type="button"
              onClick={() => workspace.navigation.select("tasks", "history")}
            >
              查看任务记录 <ArrowRightOutlined />
            </button>
          </div>
          {tasks.recentError && (
            <Alert type="warning" title={tasks.recentError} />
          )}
          <div className="rss-home-activity">
            {recent.length ? (
              recent.map((item) => (
                <button
                  type="button"
                  className="rss-home-activity-row"
                  data-task-reference={taskKey(item.reference)}
                  key={taskKey(item.reference)}
                  onClick={() => {
                    workspace.navigation.select("tasks", "history");
                    void workspace.tasks.openTask(item.reference);
                  }}
                >
                  <span className="rss-home-activity-icon">
                    {item.stage === "verified" ? (
                      <CheckOutlined />
                    ) : (
                      <ClockCircleOutlined />
                    )}
                  </span>
                  <span className="rss-grow">
                    <strong>{taskTitle(item)}</strong>
                    <small>{taskPhase[item.stage] ?? "状态待核实"}</small>
                  </span>
                  <time>
                    {item.recordedAt != null
                      ? new Date(item.recordedAt).toLocaleTimeString([], {
                          hour: "2-digit",
                          minute: "2-digit",
                        })
                      : "时间未提供"}
                  </time>
                </button>
              ))
            ) : tasks.recentLoading ? (
              <StatePanel kind="loading" title="正在读取最近动态" />
            ) : (
              <StatePanel
                kind={tasks.recentError ? "unknown" : "empty"}
                title={tasks.recentLoaded ? "暂无最近任务" : "最近任务尚未读取"}
                description="已受理的设备操作将在这里保留原任务记录。"
                onRead={() => void workspace.tasks.loadRecent()}
              />
            )}
          </div>
          {tasks.recentWindowed && (
            <p className="rss-home-source">当前仅显示已读取的近期记录。</p>
          )}
        </section>
      </div>
      <div className="rss-home-device-note">
        <DesktopOutlined />
        <span>
          {attributes?.computerName ?? "设备名称未读取"} ·{" "}
          {attributes?.ipAddresses == null
            ? "地址未读取"
            : attributes.ipAddresses.length
              ? attributes.ipAddresses.join("、")
              : "没有已读取的网络地址"}
          {device.readAt != null
            ? ` · 观察于 ${new Date(device.readAt).toLocaleTimeString()}`
            : ""}
        </span>
        <Button type="link" size="small" onClick={read}>
          重新读取
        </Button>
        <button
          type="button"
          onClick={() =>
            workspace.navigation.select("settings", "registration")
          }
        >
          设备注册与 MDM <ArrowRightOutlined />
        </button>
      </div>
    </div>
  );
}
function RegistrationBadge({
  owner,
}: {
  owner: ReturnType<typeof createRegistration>;
}) {
  const state = useSnapshot(owner.state);
  return (
    <Tag color={state.mdm?.state === "this_organization" ? "green" : "default"}>
      {state.mdm?.state === "this_organization"
        ? "由当前组织管理"
        : state.registration?.registered
          ? "设备已登记"
          : state.loading
            ? "正在读取设备状态"
            : "设备管理状态未确认"}
    </Tag>
  );
}
