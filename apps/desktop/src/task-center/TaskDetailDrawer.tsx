import { useEffect, useRef, useState } from "react";
import { Alert, Button, Modal, Progress, Tag, Timeline } from "antd";
import { useSnapshot } from "valtio";
import { deepClone } from "valtio/vanilla/utils";
import { DetailDrawer, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type { TaskAction, TaskDetail } from "../self-service/types";
import { ExecutionFacts, PolicyFacts } from "./PreparationFacts";
import { taskKey, taskTitle } from "./controller";
import {
  actionLabels,
  activityLabel,
  detectionLabel,
  nodeLabel,
  presentTask,
  recordedTime,
  referenceLabel,
  sourceLabels,
} from "./presentation";
export function TaskDetailDrawer({
  workspace,
  open,
  onReturnFocus,
}: {
  workspace: Workspace;
  open: boolean;
  onReturnFocus?(): void;
}) {
  const owner = workspace.tasks,
    state = useSnapshot(owner.state),
    task = state.selected,
    [intent, setIntent] = useState<{
      task: TaskDetail;
      action: TaskAction;
    } | null>(null),
    trigger = useRef<HTMLElement | null>(null),
    now = useTaskClock(open),
    display = task ? presentTask(task) : null;
  useEffect(() => {
    if (open)
      trigger.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    else setIntent(null);
  }, [open]);
  const close = () => {
      setIntent(null);
      owner.closeTask();
    },
    read = () => {
      const reference = owner.state.requested;
      if (reference)
        void owner.openTask(deepClone(reference), owner.state.node);
    },
    stale =
      !!intent &&
      (!task ||
        taskKey(task.reference) !== taskKey(intent.task.reference) ||
        task.scope !== intent.task.scope ||
        JSON.stringify(task.revision) !==
          JSON.stringify(intent.task.revision) ||
        !task.allowedActions.includes(intent.action)),
    blocked =
      state.busy ||
      state.detailLoading ||
      !!state.detailError ||
      !!state.pendingControl ||
      state.uncertain;
  return (
    <>
      <DetailDrawer
        open={open}
        onClose={close}
        title="任务详情"
        description="原请求、执行过程与核实结果"
        returnFocus={() => {
          const row = [
            ...document.querySelectorAll<HTMLButtonElement>(
              ".rss-task-table button[data-task-reference]",
            ),
          ].find(
            (button) => button.dataset.taskReference === owner.state.listFocus,
          );
          if (row) row.focus();
          else if (trigger.current?.isConnected) trigger.current.focus();
          else onReturnFocus?.();
        }}
        footer={
          <div className="rss-task-drawer-actions">
            <Button onClick={close}>关闭</Button>
            <Button
              disabled={state.detailLoading || !state.requested}
              onClick={read}
            >
              查询原请求
            </Button>
            {task?.allowedActions.map((action) => (
              <Button
                key={action}
                danger={action === "cancel"}
                disabled={blocked}
                onClick={() =>
                  setIntent({ task: deepClone(task) as TaskDetail, action })
                }
              >
                {actionLabels[action]}
              </Button>
            ))}
          </div>
        }
      >
        <div className="rss-task-detail rss-stack">
          {state.detailLoading && (
            <StatePanel kind="loading" title="正在读取原任务" />
          )}
          {!task && state.requested && (
            <p className="rss-reference">
              原任务：{referenceLabel(state.requested)}
            </p>
          )}
          {state.detailError && (
            <StatePanel
              kind="unknown"
              title={state.detailError}
              description="无法确认当前权限或状态；重新读取原任务后再操作。"
            />
          )}
          {state.error && <Alert type="warning" title={state.error} />}
          {state.pendingControl && (
            <Alert
              type="warning"
              title="原控制结果待核实"
              description={
                <>
                  <p>
                    控制请求已保留。确认受理不代表原任务已终止，也不会回滚已有外部效果。
                  </p>
                  <Button
                    disabled={state.loading || state.busy}
                    onClick={() => void owner.refresh()}
                  >
                    查询原控制请求
                  </Button>
                </>
              }
            />
          )}
          {task && display && (
            <>
              <div className="rss-task-detail-title">
                <div>
                  <h3>{taskTitle(task)}</h3>
                  <p className="rss-reference">
                    {referenceLabel(task.reference)}
                  </p>
                </div>
                <Tag color={display.color}>{display.label}</Tag>
              </div>
              <Alert
                type={
                  display.success
                    ? "success"
                    : display.attention
                      ? "warning"
                      : "info"
                }
                showIcon
                title={display.description}
              />
              <dl className="rss-task-facts">
                <dt>来源</dt>
                <dd>
                  <Tag color={task.source === "ai" ? "purple" : "default"}>
                    {sourceLabels[task.source]}
                  </Tag>
                </dd>
                <dt>记录时间</dt>
                <dd>{recordedTime(task.recordedAt)}</dd>
                <dt>原任务引用</dt>
                <dd className="rss-reference">
                  {referenceLabel(task.reference)}
                </dd>
                {task.policy && (
                  <>
                    <dt>Policy 固定版本</dt>
                    <dd className="rss-reference">
                      {task.policy.id} · {task.policy.revision}
                    </dd>
                  </>
                )}
                {task.policyRevision != null && (
                  <>
                    <dt>Policy 配置版本</dt>
                    <dd>{task.policyRevision}</dd>
                  </>
                )}
                {task.retryOf && (
                  <>
                    <dt>重试来源</dt>
                    <dd>
                      <Button
                        type="link"
                        onClick={() =>
                          void owner.openTask({
                            kind: "request",
                            request: task.retryOf!,
                          })
                        }
                      >
                        {task.retryOf}
                      </Button>
                    </dd>
                  </>
                )}
              </dl>
              {task.facts.kind === "execution" && (
                <>
                  <ExecutionFacts details={task.facts.value} />
                  {task.facts.value.status.cancelRequested && (
                    <Alert
                      type="warning"
                      title="已请求取消"
                      description="终止回执与效果仍由执行服务核对；已经发生的外部效果不会因此自动回滚。"
                    />
                  )}
                  <dl className="rss-task-facts">
                    <dt>停止请求回执</dt>
                    <dd>
                      {task.facts.value.status.stopOutcome === "acknowledged"
                        ? "停止请求已受理，实际终止仍需核实"
                        : task.facts.value.status.stopOutcome === "failed"
                          ? "停止请求失败"
                          : "尚未取得或不适用；不代表已终止"}
                    </dd>
                    <dt>进程退出</dt>
                    <dd>
                      {task.facts.value.status.process?.exitCode == null
                        ? "未取得退出码"
                        : `退出码 ${task.facts.value.status.process.exitCode}，不代表效果核实成功`}
                    </dd>
                    <dt>执行范围终止</dt>
                    <dd>
                      {task.facts.value.status.process?.quiescent === true
                        ? "原执行器确认静止"
                        : "未取得完整范围终止证明"}
                    </dd>
                  </dl>
                </>
              )}
              {task.facts.kind === "preparation" && (
                <>
                  <PolicyFacts
                    record={task.facts.value}
                    parameterSummary={task.parameterSummary}
                  />
                  {task.humanConfirmation && (
                    <Alert
                      type="info"
                      title="原动作需要本人核对"
                      description="请在原 AI 动作卡片核对冻结参数与期限后确认；本人确认仍不能覆盖企业拒绝或 OS 权限。"
                    />
                  )}
                  {task.facts.value.action.kind === "offered" && (
                    <Button
                      type="primary"
                      disabled={
                        blocked ||
                        !task.humanConfirmation ||
                        task.facts.value.state !== "awaitingConfirmation" ||
                        task.facts.value.action.offer.expiresAt * 1000 <= now
                      }
                      onClick={() => {
                        const selected = owner.state.selected;
                        if (
                          selected?.humanConfirmation &&
                          selected.facts.kind === "preparation" &&
                          selected.facts.value.action.kind === "offered"
                        )
                          void owner.confirmPreparation(
                            deepClone(selected.facts.value.action.offer),
                          );
                      }}
                    >
                      确认原准备的具体动作
                    </Button>
                  )}
                </>
              )}
              {task.facts.kind === "management" && (
                <dl className="rss-task-facts">
                  <dt>管理来源</dt>
                  <dd>{task.facts.value.source}</dd>
                  <dt>原活动类型</dt>
                  <dd>{task.facts.value.kind}</dd>
                  <dt>原活动阶段</dt>
                  <dd>{task.facts.value.phase}</dd>
                  <dt>来源观察时间</dt>
                  <dd>{recordedTime(task.facts.value.observedAt * 1000)}</dd>
                  <dt>投递事实</dt>
                  <dd>{activityLabel(task.facts.value.delivery)}</dd>
                  <dt>执行事实</dt>
                  <dd>{activityLabel(task.facts.value.execution)}</dd>
                  <dt>效果核实</dt>
                  <dd>{activityLabel(task.facts.value.effect)}</dd>
                  <dt>报告回执</dt>
                  <dd>{activityLabel(task.facts.value.reporting)}</dd>
                  <dt>准入或取消决定</dt>
                  <dd>{activityLabel(task.facts.value.decision)}</dd>
                  <dt>审计结果</dt>
                  <dd>
                    {task.facts.value.auditOutcome}；审计写入不代表设备效果。
                  </dd>
                  <dt>管理目标</dt>
                  <dd className="rss-reference">
                    {task.facts.value.target
                      ? `${task.facts.value.target.device} · 注册 ${task.facts.value.target.registration} · 世代 ${task.facts.value.target.generation}`
                      : "此来源未提供单设备目标"}
                  </dd>
                </dl>
              )}
              {task.facts.kind === "sampling" && (
                <dl className="rss-task-facts">
                  <dt>采集定义</dt>
                  <dd>{task.facts.value.definition.title}</dd>
                  <dt>采集来源</dt>
                  <dd>{task.facts.value.definition.kind}</dd>
                  <dt>采集时间</dt>
                  <dd>
                    {task.facts.value.latest.collectedAt == null
                      ? "未取得"
                      : recordedTime(
                          task.facts.value.latest.collectedAt * 1000,
                        )}
                  </dd>
                  <dt>缓存状态</dt>
                  <dd>
                    {task.facts.value.latest.stale
                      ? "已过期，需重新核对"
                      : "原来源观察"}
                  </dd>
                </dl>
              )}
              <div>
                <h3>处理过程</h3>
                <p className="rss-muted">
                  只显示原服务保留的节点；未提供的步骤或时间不补造。
                </p>
                {task.nodes.length ? (
                  <Timeline
                    items={task.nodes.map((node) => ({
                      key: node.id,
                      color: state.node === node.id ? "blue" : "gray",
                      title: <strong>{nodeLabel(node)}</strong>,
                      content: (
                        <div className="rss-task-node" data-node={node.id}>
                          <p className="rss-reference">{node.id}</p>
                          {node.parent && <p>所属节点：{node.parent}</p>}
                          {node.attempt && (
                            <p>
                              原执行尝试：
                              <span className="rss-reference">
                                {node.attempt}
                              </span>
                            </p>
                          )}
                          {node.reason && <p>{node.reason}</p>}
                          {node.bytes != null && (
                            <div>
                              <p>
                                已观察 {node.bytes.toLocaleString()} 字节
                                {node.total == null
                                  ? "；总量未知"
                                  : ` / ${node.total.toLocaleString()} 字节`}
                              </p>
                              {node.total != null && node.total > 0 && (
                                <Progress
                                  percent={Math.min(
                                    100,
                                    Math.round((node.bytes / node.total) * 100),
                                  )}
                                  showInfo={false}
                                />
                              )}
                            </div>
                          )}
                          {node.exitCode != null && (
                            <p>
                              进程退出码：{node.exitCode}；不作为效果成功证据。
                            </p>
                          )}
                          {node.quiescent != null && (
                            <p>
                              {node.quiescent
                                ? "此调用已确认静止"
                                : "此调用的完整终止尚未确认"}
                            </p>
                          )}
                          {node.detected && (
                            <p>{detectionLabel(node.detected)}</p>
                          )}
                        </div>
                      ),
                    }))}
                  />
                ) : (
                  <StatePanel
                    kind="empty"
                    title="原来源尚未保留处理节点"
                    description="不会由阶段名称构造一条虚拟时间线。"
                  />
                )}
              </div>
              <div>
                <h3>独立核实事实</h3>
                <dl className="rss-task-facts">
                  <dt>软件观察</dt>
                  <dd>
                    {task.softwareState
                      ? detectionLabel(task.softwareState)
                      : "未取得或不适用"}
                  </dd>
                  <dt>采集质量</dt>
                  <dd>{task.collectionQuality ?? "未取得或不适用"}</dd>
                  <dt>枚举完整</dt>
                  <dd>
                    {task.collectionComplete === true
                      ? "已确认完整"
                      : task.collectionComplete === false
                        ? "已确认不完整"
                        : "未取得或不适用"}
                  </dd>
                  <dt>上传回执</dt>
                  <dd>
                    {task.uploadAcknowledged === true
                      ? "已取得独立回执"
                      : task.uploadAcknowledged === false
                        ? "尚未确认上传"
                        : "未取得或不适用"}
                  </dd>
                </dl>
              </div>
              {task.controlReason && (
                <Alert type="info" title={task.controlReason} />
              )}
              {task.missing.length > 0 && (
                <Alert
                  type="warning"
                  title="当前记录存在关联缺口"
                  description={task.missing.join("；")}
                />
              )}
              {task.related.length > 0 && (
                <div>
                  <h3>原关联记录</h3>
                  {task.related.map((reference) => (
                    <Button
                      key={taskKey(reference)}
                      type="link"
                      onClick={() => void owner.openTask(deepClone(reference))}
                    >
                      {referenceLabel(reference)}
                    </Button>
                  ))}
                </div>
              )}
              <details>
                <summary>查看原始状态信息</summary>
                <dl className="rss-task-facts">
                  <dt>原阶段</dt>
                  <dd>{task.stage}</dd>
                  <dt>控制版本</dt>
                  <dd className="rss-reference">
                    {JSON.stringify(task.revision)}
                  </dd>
                  <dt>原引用</dt>
                  <dd className="rss-reference">
                    {JSON.stringify(task.reference)}
                  </dd>
                </dl>
              </details>
              <p className="rss-muted">
                关闭详情不会取消任务。重查原请求不会创建新任务；重试仅对服务明确允许的已知失败操作开放，并保留原任务关联。
              </p>
            </>
          )}
        </div>
      </DetailDrawer>
      <Modal
        open={!!intent && open}
        title={intent ? `${actionLabels[intent.action]}？` : "核对原任务操作"}
        onCancel={() => setIntent(null)}
        okText={intent ? actionLabels[intent.action] : "确认"}
        cancelText="返回核对"
        confirmLoading={state.busy}
        okButtonProps={{ disabled: blocked || stale }}
        onOk={() => {
          if (!intent || blocked || stale) return;
          const original = intent;
          setIntent(null);
          void owner.control(original.task, original.action);
        }}
        destroyOnHidden
      >
        {intent && (
          <>
            <p>{taskTitle(intent.task)}</p>
            <p className="rss-reference">
              {referenceLabel(intent.task.reference)}
            </p>
            <p>
              {intent.action === "cancel"
                ? "向原任务发送取消请求；实际终止与已有外部效果分别核实，不承诺回滚。"
                : intent.action === "retry"
                  ? "仅重试服务允许的已知失败；服务决定是否创建关联的新请求。未知结果应先查询原请求。"
                  : "只控制此任务的内容准备，不改变其它任务或企业授权。"}
            </p>
            {stale && (
              <Alert
                type="warning"
                title="原任务版本或权限已变化，请返回后重新核对。"
              />
            )}
          </>
        )}
      </Modal>
    </>
  );
}
function useTaskClock(active: boolean) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [active]);
  return now;
}
