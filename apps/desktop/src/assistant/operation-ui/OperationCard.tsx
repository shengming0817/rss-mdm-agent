import {
  useEffect,
  useLayoutEffect,
  useState,
  useSyncExternalStore,
  useCallback,
} from "react";
import { Alert, Button, Tag } from "antd";
import { useSnapshot } from "valtio";
import { effect, getValue } from "@a2ui/web_core/v0_9";
import {
  createOperationController,
  type OperationController,
} from "@rss-mdm-agent/ai-ui-bridge";
import { createOperationSurface } from "@rss-mdm-agent/ai-ui-bridge/surface";
import { StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../../state/workspace";
import {
  PolicyFacts,
  ExecutionFacts,
} from "../../task-center/PreparationFacts";
import { connectOperation } from "./host";
import { CardBoundary, NodeView } from "./nodes";

type Connection = Awaited<ReturnType<typeof connectOperation>>;
type Mounted = {
  owner: Workspace["assistant"];
  session: string;
  operation: string;
  generation: string;
  connection: Connection;
  controller: OperationController;
  surface: ReturnType<typeof createOperationSurface>;
};
/** A single concrete action binds the existing trusted host and its original journal record. */
export function OperationCard({
  workspace,
  operationId,
  generation,
  active,
  live,
}: {
  workspace: Workspace;
  operationId: string;
  generation: string;
  active: boolean;
  live: boolean;
}) {
  const [loaded, setLoaded] = useState<Mounted>(),
    [error, setError] = useState(""),
    [attempt, setAttempt] = useState(0),
    owner = workspace.assistant,
    selected = owner.state.selected;
  useEffect(() => {
    const abort = new AbortController();
    let mounted: Mounted | undefined;
    setLoaded(undefined);
    setError("");
    void connectOperation(owner, operationId, abort.signal)
      .then((connection) => {
        if (abort.signal.aborted) return;
        const controller = createOperationController(
          connection.presentation,
          connection.snapshot,
          connection.host,
          owner.operationDrafts,
        );
        try {
          const surface = createOperationSurface(controller.presentation);
          mounted = {
            owner,
            session: selected,
            operation: operationId,
            generation,
            connection,
            controller,
            surface,
          };
          setLoaded(mounted);
        } catch (error) {
          controller.dispose();
          throw error;
        }
      })
      .catch(() => {
        if (!abort.signal.aborted)
          setError(
            "原操作的来源或状态尚未核实，当前只保留对话文本。请重新读取原操作。",
          );
      });
    return () => {
      abort.abort();
      mounted?.controller.dispose();
      mounted?.surface.dispose();
    };
  }, [owner, selected, operationId, generation, attempt]);
  const current =
    loaded?.owner === owner &&
    loaded.session === selected &&
    loaded.operation === operationId &&
    loaded.generation === generation
      ? loaded
      : undefined;
  const reload = () => {
    setLoaded(undefined);
    setAttempt((value) => value + 1);
  };
  return (
    <section className="rss-operation-card" aria-label="具体操作核对">
      {!current && !error && (
        <StatePanel kind="loading" title="正在读取原操作" />
      )}
      {error && <StatePanel kind="unknown" title={error} onRead={reload} />}
      {current && (
        <LoadedOperation
          workspace={workspace}
          mounted={current}
          active={active}
          live={live}
          onReload={reload}
        />
      )}
    </section>
  );
}
function LoadedOperation({
  workspace,
  mounted,
  active,
  live,
  onReload,
}: {
  workspace: Workspace;
  mounted: Mounted;
  active: boolean;
  live: boolean;
  onReload(): void;
}) {
  const { controller, connection, surface } = mounted,
    state = useSnapshot(controller.state),
    subscribe = useCallback(
      (notify: () => void) =>
        effect(() => {
          getValue(surface.root);
          notify();
        }),
      [surface],
    ),
    read = useCallback(() => getValue(surface.root), [surface]),
    node = useSyncExternalStore(subscribe, read),
    raw = connection.current(),
    [failed, setFailed] = useState(false);
  useLayoutEffect(() => {
    surface.update({ ...state.draft.values });
  }, [surface, state.draft.values]);
  useEffect(() => {
    if (
      !active ||
      !live ||
      !["waiting", "running", "submitting", "pending_effect"].includes(
        state.snapshot.phase,
      )
    )
      return;
    const timer = setInterval(() => {
      if (!document.hidden && controller.can("refresh"))
        void controller.act("refresh");
    }, 2500);
    return () => clearInterval(timer);
  }, [active, live, controller, state.snapshot.phase]);
  useEffect(() => {
    if (!live) controller.invalidate();
  }, [controller, live]);
  return (
    <>
      <header className="rss-operation-heading">
        <div>
          <span className="rss-ai-card-eyebrow">具体操作</span>
          <h3>{state.snapshot.presentation.title}</h3>
        </div>
        <Tag>
          {state.snapshot.phase === "unknown" || state.unknown
            ? "结果待核实"
            : "原服务记录"}
        </Tag>
      </header>
      {raw.facts.kind === "preparation" && (
        <details className="rss-ai-preparation-facts">
          <summary>核对原准备、版本与发起来源</summary>
          <PolicyFacts
            record={raw.facts.value}
            parameterSummary={raw.parameterSummary}
          />
        </details>
      )}
      {raw.facts.kind === "execution" && (
        <details className="rss-ai-execution-facts">
          <summary>核对执行范围与来源</summary>
          <ExecutionFacts details={raw.facts.value} />
        </details>
      )}
      {!live && (
        <StatePanel
          kind="readonly"
          title="AI 会话未连接，原操作暂为只读"
          description="设备任务仍可在任务中心核对；恢复会话后先查询原操作。"
        />
      )}
      {live && (
        <CardBoundary
          onError={() => setFailed(true)}
          fallback={
            <div className="rss-stack">
              <Alert
                type="warning"
                title="操作界面未能显示，保留原事实，仅可查询。"
              />
              <p>
                {state.snapshot.result ||
                  state.snapshot.reason ||
                  "当前没有新的执行结果。"}
              </p>
              <Button
                disabled={!controller.can("refresh")}
                onClick={() => void controller.act("refresh")}
              >
                查询原操作
              </Button>
              <Button onClick={onReload}>重新加载此操作界面</Button>
            </div>
          }
        >
          {!failed && node && (
            <NodeView node={node} context={{ kind: "operation", controller }} />
          )}
        </CardBoundary>
      )}
      <footer className="rss-ai-operation-footer">
        <span>确认只对应这一项动作；执行结束与实际效果分别核实。</span>
        <Button
          type="link"
          size="small"
          onClick={() => {
            workspace.navigation.select("tasks", "history");
            void workspace.tasks.openTask(raw.reference);
          }}
        >
          查看原任务
        </Button>
      </footer>
    </>
  );
}
