import { useState } from "react";
import { Alert, Button, Table } from "antd";
import { deepClone } from "valtio/vanilla/utils";
import type { Snapshot } from "valtio/vanilla";
import { PageHeader } from "@rss-mdm-agent/ui";
import type { ManagementActivity } from "../self-service/types";
import type { ManagementEntry } from "../state/navigation";
import { nativeNavigation } from "../workspace/entry";
import { activityLabel, recordedTime } from "./presentation";

export function ManagementEntryPanel({
  entry,
  onClose,
}: {
  entry: Snapshot<ManagementEntry>;
  onClose(): void;
}) {
  const [page, setPage] = useState(
    entry.kind === "batch" ? deepClone(entry.value.page) : null,
  );
  const [loading, setLoading] = useState(false);
  const [failure, setFailure] = useState<"stale" | "unavailable" | null>(null);
  const selector = entry.kind === "batch" ? entry.value.intent.selector : null;

  async function read(after: string | null) {
    if (entry.kind !== "batch" || loading) return;
    setLoading(true);
    try {
      const value = await nativeNavigation()?.({
        kind: "resolve",
        intent: deepClone(entry.value.intent),
        after,
      });
      if (value?.kind !== "activities") throw Error("Invalid activity page");
      setPage(value.value.page);
      setFailure(null);
    } catch (error) {
      setPage(null);
      setFailure(
        typeof error === "object" &&
          error !== null &&
          "code" in error &&
          error.code === "navigation_cursor_stale"
          ? "stale"
          : "unavailable",
      );
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="rss-page rss-tasks-page">
      <PageHeader
        title={entry.kind === "event" ? "共享事件" : "管理活动批次"}
        description={
          entry.kind === "event"
            ? entry.name
            : selector?.kind === "batch"
              ? selector.id
              : ""
        }
        actions={<Button onClick={onClose}>返回管理活动</Button>}
      />
      {failure && (
        <Alert
          type="warning"
          title={
            failure === "stale"
              ? "批次分页已失效，请重新读取批次。"
              : "无法读取原批次，请重新核对当前注册与权限。"
          }
        />
      )}
      {page && (
        <>
          <p>读取时间：{recordedTime(page.asOf * 1000)}</p>
          {page.coverage.map((source) => (
            <p key={source.source}>
              {source.source} · {source.complete ? "覆盖完整" : "覆盖不完整"}
              {source.reasons.length ? ` · ${source.reasons.join("；")}` : ""}
            </p>
          ))}
          <Table<Snapshot<ManagementActivity>>
            rowKey="id"
            pagination={false}
            dataSource={page.items}
            locale={{ emptyText: "本页没有可见的批次活动" }}
            columns={[
              { title: "原活动", dataIndex: "id" },
              {
                title: "原关联",
                render: (_, { links }) =>
                  [
                    links.member,
                    links.policy,
                    links.task,
                    links.operation,
                    links.retirement,
                  ]
                    .filter(Boolean)
                    .join(" · ") || "—",
              },
              { title: "阶段", render: (_, fact) => activityLabel(fact.phase) },
              {
                title: "受理",
                render: (_, fact) => activityLabel(fact.decision ?? null),
              },
              {
                title: "执行",
                render: (_, fact) => activityLabel(fact.execution ?? null),
              },
              {
                title: "效果",
                render: (_, fact) => activityLabel(fact.effect ?? null),
              },
              {
                title: "记录时间",
                render: (_, fact) => recordedTime(fact.recordedAt * 1000),
              },
            ]}
          />
        </>
      )}
      {entry.kind === "batch" && (
        <>
          <Button loading={loading} onClick={() => void read(null)}>
            重新读取批次
          </Button>
          {page?.nextCursor && (
            <Button
              loading={loading}
              onClick={() => void read(page.nextCursor ?? null)}
            >
              下一页
            </Button>
          )}
        </>
      )}
    </div>
  );
}
