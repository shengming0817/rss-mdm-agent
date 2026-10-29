import { localActionLabels } from "./presentation";
import { Alert, Button, Tag } from "antd";
import { useSnapshot } from "valtio";
import { DetailDrawer, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { canSubmitSoftware } from "./controller";
import { SoftwareFacts } from "../task-center/PreparationFacts";
import { taskPhase } from "../task-center/controller";
export function SoftwarePreparation({
  workspace,
  request,
  onClose,
}: {
  workspace: Workspace;
  request: string | null;
  onClose(): void;
}) {
  const state = useSnapshot(workspace.software.state),
    prepared = request ? state.prepared.get(request) : undefined,
    status = request ? state.operations.get(request) : undefined,
    busy = !!request && state.busy.has(request),
    canSubmit =
      canSubmitSoftware(status, state.now) &&
      !!request &&
      !state.uncertain.has(request);
  return (
    <DetailDrawer
      open={!!request}
      onClose={onClose}
      title={prepared?.name ?? "查询原软件准备"}
      description="核对服务固定的操作对象、更新版本与执行身份。"
    >
      <div className="rss-stack">
        {state.error && <Alert type="warning" title={state.error} />}
        {prepared ? (
          <>
            <Tag>{localActionLabels[prepared.choice.action]}</Tag>
            <SoftwareFacts prepared={prepared} />
            {prepared.startBeforeUnixMs <= state.now && (
              <Alert
                type="warning"
                title="原软件准备已过期，不能沿用旧确认；可继续查询原请求。"
              />
            )}
            <p>
              当前原状态：
              {status ? taskPhase[status.execution.phase] : "尚未确认"}
            </p>
            <Button
              type="primary"
              disabled={
                !canSubmit || busy || prepared.startBeforeUnixMs <= state.now
              }
              onClick={() => {
                const original = workspace.software.state.prepared.get(
                  prepared.request,
                );
                if (original) void workspace.software.submit(original);
              }}
            >
              {`确认${localActionLabels[prepared.choice.action]}`}
            </Button>
          </>
        ) : (
          <StatePanel
            kind="unknown"
            title="准备回执尚未取得"
            description="按原请求重新读取，不会创建新操作。"
          />
        )}
        {status?.execution.cancelRequested && (
          <Alert
            type="warning"
            title="已请求取消，终止与外部效果仍待原服务核实。"
          />
        )}
        <Button
          disabled={busy}
          onClick={() => {
            if (request) void workspace.software.query(request).catch(() => {});
          }}
        >
          查询原请求
        </Button>
        <Button
          onClick={() => {
            if (request) {
              onClose();
              workspace.navigation.select("tasks", "history");
              void workspace.tasks.openTask({ kind: "request", request });
            }
          }}
        >
          查看原任务
        </Button>
        <p className="rss-muted">
          关闭面板不会撤销已提交操作；受理、执行结束与核实结果分别记录。
        </p>
      </div>
    </DetailDrawer>
  );
}
