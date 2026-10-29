import { useEffect, useState } from "react";
import { Alert, Button, Input, InputNumber, Tag } from "antd";
import { useSnapshot } from "valtio";
import { DetailDrawer } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { taskPhase } from "../task-center/controller";
const findings = {
  reachable: "可连接",
  notRequired: "IP 地址无需 DNS 查询",
  dnsFailed: "DNS 解析失败",
  refused: "连接被拒绝",
  unreachable: "目标不可达",
  timedOut: "连接超时",
};
export function NetworkDrawer({
  workspace,
  open,
  onClose,
}: {
  workspace: Workspace;
  open: boolean;
  onClose(): void;
}) {
  const owner = workspace.networkCheck,
    state = useSnapshot(owner.state),
    nav = useSnapshot(workspace.navigation.state),
    [host, setHost] = useState(""),
    [port, setPort] = useState<number | null>(443),
    [dirty, setDirty] = useState(false),
    blocked = !!state.request && !owner.ended();
  useEffect(() => {
    if (open) void owner.load();
  }, [open, owner]);
  useEffect(() => {
    const input = nav.preparation?.input;
    if (!open || input?.kind !== "network") return;
    if (!blocked && input.target) {
      setHost(input.target.host);
      setPort(input.target.port);
      setDirty(true);
    }
    workspace.navigation.clearPreparation();
  }, [open, nav.preparation]);
  useEffect(() => {
    if (!dirty && state.defaults) {
      setHost(state.defaults.host);
      setPort(state.defaults.port);
    }
  }, [state.defaults, dirty]);
  const valid =
    host.trim().length > 0 &&
    !/[\s/@?#]/.test(host) &&
    !host.includes("://") &&
    port != null &&
    Number.isInteger(port) &&
    port > 0 &&
    port <= 65535;
  return (
    <DetailDrawer
      open={open}
      onClose={onClose}
      title="网络连通性检查"
      description="针对明确主机与端口检查 DNS 和 TCP，不测试 HTTPS 或应用服务健康。"
    >
      <div className="rss-stack">
        {!owner.available && (
          <Alert type="info" title="当前装配未提供原生网络检测能力。" />
        )}
        {state.error && <Alert type="warning" title={state.error} />}
        <label className="rss-field">
          <span>主机名或 IP 地址</span>
          <Input
            aria-label="网络检测主机"
            placeholder="service.example.com"
            disabled={state.busy || blocked}
            value={host}
            onChange={(event) => {
              setDirty(true);
              setHost(event.target.value);
            }}
          />
        </label>
        <label className="rss-field">
          <span>TCP 端口</span>
          <InputNumber
            aria-label="网络检测端口"
            min={1}
            max={65535}
            precision={0}
            disabled={state.busy || blocked}
            value={port}
            onChange={(value) => {
              setDirty(true);
              setPort(value);
            }}
          />
        </label>
        <Button
          type="primary"
          disabled={!owner.available || state.busy || blocked || !valid}
          onClick={() => {
            if (port) void owner.run({ host: host.trim(), port });
          }}
        >
          开始检测
        </Button>
        {state.request && (
          <>
            <p className="rss-reference">原请求：{state.request}</p>
            <p>
              固定目标：{state.target?.host}:{state.target?.port}
            </p>
            <Tag>
              {state.status
                ? taskPhase[state.status.status.phase]
                : "结果尚未确认"}
            </Tag>
            <Button disabled={state.busy} onClick={() => void owner.recover()}>
              查询原检测
            </Button>
            <Button
              onClick={() => {
                const request = owner.state.request;
                if (request) {
                  onClose();
                  workspace.navigation.select("tasks", "history");
                  void workspace.tasks.openTask({ kind: "request", request });
                }
              }}
            >
              查看原任务
            </Button>
          </>
        )}
        {state.status?.result && (
          <section>
            <h3>实际检测结果</h3>
            <p className="rss-muted">
              观察于{" "}
              {new Date(state.status.result.collectedAtUnixMs).toLocaleString()}
            </p>
            <div className="rss-setting-row">
              <span>DNS</span>
              <span>
                {findings[state.status.result.dns.finding]} ·{" "}
                {state.status.result.dns.durationMs} ms
              </span>
            </div>
            {state.status.result.tcp.map((probe, index) => (
              <div
                className="rss-setting-row"
                key={`${probe.address}:${index}`}
              >
                <span>{probe.address ?? `TCP 尝试 ${index + 1}`}</span>
                <span>
                  {findings[probe.finding]} · {probe.durationMs} ms
                </span>
              </div>
            ))}
          </section>
        )}
        <p className="rss-muted">
          检测结果未确认时保留原请求与目标。关闭面板不会取消服务中的操作。
        </p>
      </div>
    </DetailDrawer>
  );
}
