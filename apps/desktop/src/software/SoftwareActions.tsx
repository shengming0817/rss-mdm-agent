import { useEffect, useRef, useState } from "react";
import { Alert, Button, Modal, Tag } from "antd";
import { useSnapshot } from "valtio";
import type { Snapshot } from "valtio/vanilla";
import type { Workspace } from "../state/workspace";
import type {
  InstalledSoftware,
  LocalSoftwareAction,
  SelfServiceDirectoryItem,
  TaskDetail,
} from "../self-service/types";
import { canSubmitSoftware } from "./controller";
import { localAction, localActionLabels } from "./presentation";
import { taskPhase } from "../task-center/controller";
import { presentTask } from "../task-center/presentation";

/** A click is consent for this exact software action; service admission still owns execution. */
export function SoftwareActions({
  workspace,
  installed,
  policy,
  updates = false,
  active = true,
}: {
  workspace: Workspace;
  installed?: Snapshot<InstalledSoftware>;
  policy?: Snapshot<SelfServiceDirectoryItem>;
  updates?: boolean;
  active?: boolean;
}) {
  const software = useSnapshot(workspace.software.state);
  const policies = useSnapshot(workspace.policies.state);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [uninstall, setUninstall] = useState(false);
  const [detail, setDetail] = useState<TaskDetail | null>(null);
  const lock = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const previousOperation = installed
    ? [...software.operations.values()]
        .filter(
          (value) =>
            JSON.stringify(value.prepared.choice.software) ===
            JSON.stringify(installed.reference),
        )
        .at(-1)
    : undefined;
  const selectedPolicy =
    !!policy && policies.item?.policyId === policy.policyId;
  const request =
    (installed
      ? (software.installations.get(installed.reference.installation) ??
        previousOperation?.prepared.request)
      : policy
        ? software.policyRequests.get(policy.policyId)
        : undefined) ??
    (selectedPolicy ? policies.submission?.choice.request : undefined);
  const operation = request ? software.operations.get(request) : undefined;
  const status = operation?.execution;
  // Poll facts only. Never re-submit a request when an acknowledgement is missing.
  useEffect(() => {
    if (!request || !active) return;
    setDetail(null);
    let current = true;
    let timer: ReturnType<typeof setTimeout>;
    const read = async () => {
      try {
        if (installed) {
          const value = await workspace.software.query(request);
          if (
            value &&
            [
              "verified",
              "cancelled",
              "admissionDenied",
              "failedBeforeDispatch",
            ].includes(value.execution.phase)
          ) {
            if (current) setError("");
            return;
          }
        } else {
          if (workspace.policies.state.submission?.choice.request === request)
            await workspace.policies.recover();
          const value = await workspace.tasks.detail(
            { kind: "request", request },
            true,
          );
          if (current) setDetail(value);
          if (
            [
              "verified",
              "cancelled",
              "admissionDenied",
              "failedBeforeDispatch",
              "failed",
            ].includes(value.stage)
          ) {
            if (current) setError("");
            return;
          }
        }
        if (current) setError("");
      } catch {
        if (current) {
          setDetail(null);
          setError("暂时无法获取进度，请稍后查询。请勿重复操作。");
        }
      }
      if (current) timer = setTimeout(read, 3000);
    };
    void read();
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [workspace, request, active, installed?.reference.installation]);
  const label = status
    ? status.phase === "verified"
      ? status.assessment === "satisfied"
        ? "已完成"
        : "结果需关注"
      : taskPhase[status.phase]
    : detail
      ? presentTask(detail).label
      : request
        ? "正在获取进度"
        : "";
  const pending =
    (request && !status && !!installed) ||
    (status &&
      ![
        "verified",
        "cancelled",
        "admissionDenied",
        "failedBeforeDispatch",
      ].includes(status.phase));
  async function run(action: LocalSoftwareAction = "launch") {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      if (installed) {
        const prepared = await workspace.software.prepare(installed, action);
        // An existing different action must never inherit consent from this click.
        if (prepared?.choice.action === action)
          await workspace.software.submit(prepared);
      } else if (policy) {
        const owner = workspace.policies;
        if (owner.state.submission) {
          if (owner.state.item?.policyId === policy.policyId) {
            await owner.recover();
            return;
          }
          if (!owner.clear()) throw Error("pending");
        }
        await owner.select(policy as SelfServiceDirectoryItem);
        if (mounted.current && owner.state.error) setError(owner.state.error);
        if (owner.state.item?.policyId === policy.policyId) {
          await owner.submit({});
          const original = owner.state.submission?.choice.request;
          if (original)
            workspace.software.state.policyRequests.set(
              policy.policyId,
              original,
            );
        }
      }
    } catch {
      if (mounted.current) setError("操作尚未确认，请查询进度后再试。");
    } finally {
      lock.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  async function refresh() {
    if (!request) return;
    try {
      if (installed) await workspace.software.query(request);
      else {
        if (workspace.policies.state.submission?.choice.request === request)
          await workspace.policies.recover();
        const value = await workspace.tasks.detail(
          { kind: "request", request },
          true,
        );
        if (mounted.current) setDetail(value);
      }
      if (mounted.current) setError("");
    } catch {
      if (mounted.current) {
        setDetail(null);
        setError("暂时无法获取进度，请稍后查询。");
      }
    }
  }
  const resumable =
    !!operation &&
    !busy &&
    !software.busy.has(operation.prepared.request) &&
    !software.uncertain.has(operation.prepared.request) &&
    canSubmitSoftware(operation, software.now);
  const retryPolicy =
    policy &&
    (selectedPolicy
      ? workspace.policies.canRestart(policies.status)
      : !!detail &&
        [
          "failed",
          "cancelled",
          "failedBeforeDispatch",
          "admissionDenied",
        ].includes(detail.stage));
  const failure =
    error ||
    (selectedPolicy ? policies.error : "") ||
    (installed && software.error ? "操作结果尚未确认，请查询进度。" : "");
  return (
    <div className="rss-stack">
      <div className="rss-inline-actions">
        {installed ? (
          (updates
            ? (["update"] as const)
            : ([
                "launch",
                "uninstall",
                ...(localAction(installed, "update").available
                  ? ["update" as const]
                  : []),
              ] as const)
          ).map((action) => {
            const capability = localAction(installed, action);
            return (
              <span key={action} title={capability.reason}>
                <Button
                  danger={action === "uninstall"}
                  loading={busy}
                  disabled={
                    !capability.available ||
                    !!pending ||
                    software.busy.has(installed.reference.installation)
                  }
                  onClick={() =>
                    action === "uninstall"
                      ? setUninstall(true)
                      : void run(action)
                  }
                >
                  {localActionLabels[action]}
                  {action === "update" && capability.version
                    ? `至 ${capability.version}`
                    : ""}
                </Button>
              </span>
            );
          })
        ) : (
          <Button
            type="primary"
            loading={busy || (selectedPolicy && policies.busy)}
            disabled={
              !workspace.policies.available || policies.busy || !!request
            }
            onClick={() => void run()}
          >
            安装
          </Button>
        )}
        {resumable && (
          <Button
            onClick={() => {
              const original = workspace.software.state.prepared.get(
                operation.prepared.request,
              );
              if (original) void workspace.software.submit(original);
            }}
          >
            继续{localActionLabels[operation.prepared.choice.action]}
            {operation.prepared.targetVersion
              ? `至 ${operation.prepared.targetVersion}`
              : ""}
          </Button>
        )}
        {selectedPolicy && policies.status?.kind === "absent" && (
          <Button
            disabled={policies.busy}
            onClick={() => void workspace.policies.submit({})}
          >
            重试提交
          </Button>
        )}
        {retryPolicy && (
          <Button
            disabled={policies.busy}
            onClick={async () => {
              if (selectedPolicy) await workspace.policies.restart();
              else if (!workspace.policies.clear()) {
                setError("另一项安装结果尚未确认，请先查询其进度。");
                return;
              }
              if (!workspace.policies.state.submission && policy) {
                workspace.software.state.policyRequests.delete(policy.policyId);
                void run();
              }
            }}
          >
            重新安装
          </Button>
        )}
        {request && <Button onClick={() => void refresh()}>查询进度</Button>}
      </div>
      {label && (
        <div role="status">
          <Tag>{label}</Tag>
          {status?.cancelRequested && <span>正在取消，等待确认</span>}
        </div>
      )}
      {operation &&
        ["waiting", "confirmationRequired"].includes(
          operation.execution.phase,
        ) &&
        operation.prepared.startBeforeUnixMs <= software.now && (
          <Alert type="warning" title="本次操作已过期，未提交执行。" />
        )}
      {failure && <Alert type="warning" title={failure} />}
      <Modal
        open={uninstall}
        title={`卸载 ${installed?.name ?? "软件"}？`}
        okText="卸载"
        cancelText="取消"
        onCancel={() => setUninstall(false)}
        onOk={() => {
          setUninstall(false);
          void run("uninstall");
        }}
      >
        <p>
          将从这台电脑移除此软件。请先保存工作；应用数据是否保留由卸载程序决定。
        </p>
      </Modal>
    </div>
  );
}
