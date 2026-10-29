import { useEffect, useState } from "react";
import { Alert, Button, Popconfirm, Switch, Tag } from "antd";
import { useSnapshot } from "valtio";
import {
  DetailDrawer,
  PageHeader,
  SectionCard,
  StatePanel,
} from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { createRegistration } from "../account/registration";
import { diagnosticMessage } from "./controller";
const scopeLabels = {
  host: "AI Host",
  service: "系统服务",
  device: "设备概览",
  execution: "执行诊断",
};
export function DiagnosticsPage({
  workspace,
  registration,
  active,
}: {
  workspace: Workspace;
  registration: ReturnType<typeof createRegistration> | null;
  active: boolean;
}) {
  const host = useSnapshot(workspace.host.state),
    diagnostics = useSnapshot(workspace.diagnostics.state),
    [preview, setPreview] = useState(false);
  useEffect(() => {
    if (active) {
      void workspace.host.refresh();
      void registration?.refresh();
    } else {
      workspace.diagnostics.clear();
      setPreview(false);
    }
  }, [workspace, registration, active]);
  const close = () => {
    workspace.diagnostics.clear();
    setPreview(false);
  };
  return (
    <div className="rss-page rss-preferences-page">
      <p className="rss-eyebrow">PREFERENCES</p>
      <PageHeader
        title="运行与诊断"
        description="分别查看本机系统服务与当前用户的 AI Host，按需生成脱敏摘要。"
        actions={
          <Button
            disabled={host.loading || host.busy}
            onClick={() => {
              void workspace.host.refresh();
              void registration?.refresh();
            }}
          >
            重读运行状态
          </Button>
        }
      />
      <div className="rss-stack">
        <SectionCard title="本机服务">
          {registration && <ServiceStatus owner={registration} />}
          <div className="rss-setting-row">
            <span>
              <strong>AI Host</strong>
              <small>当前用户的 AI 会话服务</small>
            </span>
            <Tag color={host.status?.phase === "ready" ? "green" : "default"}>
              {host.status?.phase === "ready"
                ? "已就绪"
                : (host.status?.phase ?? "尚未读取")}
            </Tag>
          </div>
          {host.readError && <Alert type="warning" title={host.readError} />}
          {host.message && <Alert type="info" title={host.message} />}
          {diagnosticMessage(host.status) && (
            <Alert type="warning" title={diagnosticMessage(host.status)} />
          )}
          <div className="rss-settings-actions">
            <Button
              onClick={() => workspace.navigation.select("tasks", "history")}
            >
              查看原任务中心
            </Button>
            <Popconfirm
              title="受控重启 AI Host？"
              description="可能中断当前 AI 会话；设备操作仍由系统服务继续并保留原记录。"
              onConfirm={() => void workspace.host.restart()}
            >
              <Button
                disabled={
                  !workspace.host.available ||
                  !host.status ||
                  host.busy ||
                  host.loading
                }
              >
                重启 AI Host
              </Button>
            </Popconfirm>
          </div>
        </SectionCard>
        <SectionCard title="诊断摘要">
          <p className="rss-muted">
            由本机诊断服务按所选范围生成脱敏内容；先预览，再决定是否导出。不会在页面中拼接原始凭据或会话全文。
          </p>
          <div className="rss-diagnostic-scopes">
            {(Object.keys(scopeLabels) as (keyof typeof scopeLabels)[]).map(
              (key) => (
                <label key={key}>
                  <span>{scopeLabels[key]}</span>
                  <Switch
                    aria-label={`诊断范围 ${scopeLabels[key]}`}
                    checked={diagnostics.scopes[key]}
                    disabled={diagnostics.busy}
                    onChange={(checked) => {
                      workspace.diagnostics.clear();
                      workspace.diagnostics.state.scopes[key] = checked;
                    }}
                  />
                </label>
              ),
            )}
          </div>
          {!workspace.diagnostics.available && (
            <StatePanel
              kind="readonly"
              title="当前装配未提供本机诊断导出能力"
            />
          )}
          <Button
            disabled={
              !workspace.diagnostics.available ||
              diagnostics.busy ||
              !Object.values(diagnostics.scopes).some(Boolean)
            }
            onClick={() => {
              setPreview(true);
              void workspace.diagnostics.prepare();
            }}
          >
            预览脱敏诊断
          </Button>
        </SectionCard>
      </div>
      <DetailDrawer
        open={active && preview}
        onClose={close}
        title="脱敏诊断预览"
        description="此内容来自本机诊断服务的固定快照；导出保存同一快照。"
      >
        <div className="rss-stack">
          {diagnostics.busy && (
            <StatePanel kind="loading" title="正在处理诊断快照" />
          )}
          {diagnostics.message && (
            <Alert type="info" title={diagnostics.message} />
          )}
          {diagnostics.preview && (
            <>
              <p className="rss-muted">
                脱敏快照 · {diagnostics.preview.bytes.toLocaleString()} 字节 ·
                原服务在有效期内核对
              </p>
              <pre className="rss-diagnostic-preview">
                {JSON.stringify(diagnostics.preview.content, null, 2)}
              </pre>
              <Button
                type="primary"
                disabled={diagnostics.busy}
                onClick={() => void workspace.diagnostics.save()}
              >
                保存此脱敏快照
              </Button>
            </>
          )}
          <Button
            disabled={diagnostics.busy}
            onClick={() => void workspace.diagnostics.prepare()}
          >
            重新生成预览
          </Button>
        </div>
      </DetailDrawer>
    </div>
  );
}
function ServiceStatus({
  owner,
}: {
  owner: ReturnType<typeof createRegistration>;
}) {
  const state = useSnapshot(owner.state),
    labels = {
      notInstalled: "尚未安装",
      configurationRequired: "需要配置",
      rejected: "连接被拒绝",
      unavailable: "连接不可用",
      mismatch: "版本或主体不匹配",
      connected: "已连接",
    };
  return (
    <>
      <div className="rss-setting-row">
        <span>
          <strong>设备系统服务</strong>
          <small>
            {state.service?.phase === "connected"
              ? `${state.service.status.platform} · ${state.service.status.build} · ${state.service.status.readiness.phase === "ready" ? "已就绪" : "尚未就绪"}`
              : "本机认证服务与设备执行"}
          </small>
        </span>
        <Tag color={state.service?.phase === "connected" ? "green" : "default"}>
          {state.service ? labels[state.service.phase] : "尚未读取"}
        </Tag>
      </div>
      {state.serviceError && (
        <Alert type="warning" title={state.serviceError} />
      )}
    </>
  );
}
