import { useEffect, useState } from "react";
import { Alert, Button } from "antd";
import { useSnapshot } from "valtio";
import { StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type { TaskRef } from "../self-service/types";
import { taskKey, taskPhase, taskTitle } from "../task-center/controller";
export function PolicyTasks({
  workspace,
  policy,
}: {
  workspace: Workspace;
  policy: string;
}) {
  const tasks = useSnapshot(workspace.tasks.state),
    [references, setReferences] = useState<TaskRef[]>([]),
    [next, setNext] = useState<string | null>(null),
    [loaded, setLoaded] = useState(false),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [complete, setComplete] = useState(false),
    [remote, setRemote] = useState(false),
    [asOf, setAsOf] = useState(0);
  useEffect(() => {
    let current = true;
    setBusy(true);
    void workspace.tasks
      .policyTasks(policy)
      .then((page) => {
        if (!current || !page) return;
        setReferences(page.items.map((item) => item.reference));
        setNext(page.next);
        setLoaded(true);
        setAsOf(page.asOf);
        setComplete(page.complete);
        setRemote(page.remoteAvailable);
        setError("");
      })
      .catch(() => {
        if (current)
          setError(
            "发布项目的原任务读取未完成，请重读；未返回任务不代表没有执行。",
          );
      })
      .finally(() => {
        if (current) setBusy(false);
      });
    return () => {
      current = false;
    };
  }, [workspace, policy]);
  async function read(append: boolean) {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const page = await workspace.tasks.policyTasks(
        policy,
        append ? next : null,
      );
      if (!page) return;
      setReferences((values) => [
        ...new Map(
          [
            ...(append ? values : []),
            ...page.items.map((item) => item.reference),
          ].map((reference) => [taskKey(reference), reference]),
        ).values(),
      ]);
      setNext(page.next);
      setLoaded(true);
      setAsOf(page.asOf);
      setComplete(page.complete);
      setRemote(page.remoteAvailable);
    } catch {
      setError("发布项目的原任务读取未完成，请重读。");
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="rss-stack">
      {error && <Alert type="warning" title={error} />}
      {busy && !loaded && (
        <StatePanel kind="loading" title="正在读取发布项目的原任务" />
      )}
      {references.map((reference) => {
        const item = tasks.byRef.get(taskKey(reference));
        return item ? (
          <Button
            key={taskKey(reference)}
            className="rss-related-task"
            onClick={() => {
              workspace.navigation.select("tasks", "history");
              void workspace.tasks.openTask(reference);
            }}
          >
            {taskTitle(item)} · {taskPhase[item.stage]}
            <small>{taskKey(reference)}</small>
          </Button>
        ) : null;
      })}
      {loaded && !references.length && !error && (
        <StatePanel
          kind={complete && !next ? "empty" : "unknown"}
          title="当前读取范围没有该发布项目的原任务"
          description={
            complete && !next
              ? "当前来源已读完。"
              : "覆盖尚未完整，不能据此判断没有执行。"
          }
        />
      )}
      {asOf > 0 && (
        <p className="rss-muted">
          原任务服务 · 精确 Policy {policy} · {new Date(asOf).toLocaleString()}
          {next ? " · 尚有后续记录" : " · 当前页已读完"}
          {complete ? " · 来源覆盖完整" : " · 来源覆盖未完整"}
          {remote ? " · 远端读取可用" : " · 远端当前不可用，保留结果供参考"}
        </p>
      )}
      <div className="rss-inline-actions">
        <Button disabled={busy} onClick={() => void read(false)}>
          重读发布任务
        </Button>
        {next && (
          <Button disabled={busy} onClick={() => void read(true)}>
            读取更多发布任务
          </Button>
        )}
      </div>
    </div>
  );
}
