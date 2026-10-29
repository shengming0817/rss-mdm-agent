import { useEffect, useRef, useState } from "react";
import { Alert, Button, Input, Select, Table, Tag } from "antd";
import { SearchOutlined, ReloadOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import type { Snapshot } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import { PageHeader, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type { TaskDetail, TaskSource, TaskView } from "../self-service/types";
import { taskKey, taskTitle } from "./controller";
import {
  presentTask,
  referenceLabel,
  recordedTime,
  sourceLabels,
} from "./presentation";
import "./tasks.css";
export function TaskCenterPage({
  workspace,
  section,
  active,
}: {
  workspace: Workspace;
  section: string;
  active: boolean;
}) {
  const owner = workspace.tasks,
    state = useSnapshot(owner.state),
    [search, setSearch] = useState(""),
    [source, setSource] = useState<TaskSource | null>(null),
    view: TaskView =
      section === "active" || section === "management" ? section : "history",
    applied = useRef("");
  useEffect(() => {
    const query = JSON.stringify([view, source]);
    if (active && applied.current !== query) {
      applied.current = query;
      void owner.setQuery({ view, source });
    }
  }, [active, owner, view, source]);
  const rows = state.items.filter((item) =>
    `${taskTitle(item)} ${referenceLabel(item.reference)}`
      .toLocaleLowerCase()
      .includes(search.trim().toLocaleLowerCase()),
  );
  const summary = state.items.map(presentTask),
    counts = [
      state.items.filter((item) => item.active).length,
      summary.filter((item) => item.attention).length,
      summary.filter((item) => item.success).length,
    ];
  const open = (task: Snapshot<TaskDetail>) => {
    owner.state.listFocus = taskKey(task.reference);
    void owner.openTask(deepClone(task.reference));
  };
  return (
    <div className="rss-page rss-tasks-page">
      <p className="rss-eyebrow">ACTIVITY &amp; EXECUTION</p>
      <PageHeader
        title="任务中心"
        description="操作进度、执行结果与实际核实，清楚分开。"
        actions={
          <Button
            icon={<ReloadOutlined />}
            disabled={!owner.available || state.loading}
            onClick={() => void owner.refresh()}
          >
            刷新原记录
          </Button>
        }
      />
      <div className="rss-task-summary" aria-label="本页已读取任务摘要">
        {["正在进行", "需要关注", "核实符合预期"].map((label, index) => (
          <div key={label}>
            <strong>{counts[index]}</strong>
            <span>{label}</span>
          </div>
        ))}
      </div>
      <p className="rss-muted rss-task-scope">
        摘要仅统计本页已读取的 {state.items.length} 条记录；
        {state.next
          ? "还有后续记录"
          : state.complete
            ? "当前查询覆盖完整"
            : "当前查询覆盖未完整"}
        。
      </p>
      <div className="rss-task-filterbar">
        <Input
          aria-label="搜索任务"
          prefix={<SearchOutlined />}
          placeholder="搜索本页任务名称或编号"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          allowClear
        />
        <label>
          <span>来源</span>
          <Select<TaskSource | "all">
            aria-label="任务来源"
            value={source ?? "all"}
            onChange={(value) => setSource(value === "all" ? null : value)}
            options={[
              { value: "all", label: "全部来源" },
              ...Object.entries(sourceLabels).map(([value, label]) => ({
                value,
                label,
              })),
            ]}
          />
        </label>
      </div>
      {!owner.available && (
        <StatePanel
          kind="readonly"
          title="当前装配未提供本机任务服务"
          description="任务由原执行服务持有，恢复连接后重新读取。"
        />
      )}
      {state.error && (
        <StatePanel
          kind={state.uncertain ? "unknown" : "error"}
          title={state.error}
          onRead={() => void owner.refresh()}
        />
      )}
      {state.loaded && (
        <div className="rss-task-coverage">
          <span>
            读取时间：{state.asOf ? recordedTime(state.asOf) : "尚未取得"}
          </span>
          <span>
            {state.remoteAvailable
              ? "管理来源已连接"
              : "管理来源未就绪；已读记录不代表在线事实"}
          </span>
          {state.lastSync != null && (
            <span>最近同步：{recordedTime(state.lastSync * 1000)}</span>
          )}
          {state.retainedFrom != null && (
            <span>保留起点：{recordedTime(state.retainedFrom * 1000)}</span>
          )}
          {state.coverage.map((coverage) => (
            <div key={coverage.source}>
              <Tag color={coverage.complete ? "default" : "gold"}>
                {sourceLabels[coverage.source as TaskSource] ?? coverage.source}{" "}
                · {coverage.complete ? "覆盖完整" : "覆盖不完整"}
              </Tag>
              {coverage.reasons.map((reason) => (
                <span key={reason}>{reason}</span>
              ))}
            </div>
          ))}
        </div>
      )}
      {state.pendingControl && (
        <Alert
          type="warning"
          title="原控制操作结果尚待核实"
          description={
            <>
              <p>继续查询同一控制请求，不重复发送控制。</p>
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
      {state.loading && !state.loaded ? (
        <StatePanel kind="loading" title="正在读取原任务记录" />
      ) : rows.length ? (
        <Table<Snapshot<TaskDetail>>
          rowKey={(item) => taskKey(item.reference)}
          className="rss-task-table"
          dataSource={rows}
          pagination={false}
          scroll={{ x: 640 }}
          columns={[
            {
              title: "任务 / 编号",
              key: "task",
              render: (_, task) => (
                <div>
                  <strong>{taskTitle(task)}</strong>
                  <small className="rss-reference">
                    {referenceLabel(task.reference)}
                  </small>
                </div>
              ),
            },
            {
              title: "来源",
              key: "source",
              width: 95,
              render: (_, task) => (
                <Tag color={task.source === "ai" ? "purple" : "default"}>
                  {sourceLabels[task.source]}
                </Tag>
              ),
            },
            {
              title: "状态",
              key: "status",
              render: (_, task) => {
                const display = presentTask(task);
                return <Tag color={display.color}>{display.label}</Tag>;
              },
            },
            {
              title: "记录时间",
              key: "recorded",
              render: (_, task) => recordedTime(task.recordedAt),
            },
            {
              title: "操作",
              key: "controls",
              width: 110,
              render: (_, task) => (
                <Button
                  data-task-reference={taskKey(task.reference)}
                  type="link"
                  size="small"
                  onClick={() => open(task)}
                >
                  查看详情
                </Button>
              ),
            },
          ]}
        />
      ) : (
        !state.error && (
          <StatePanel
            kind="empty"
            title={search ? "本页没有匹配的任务" : "当前视图没有任务"}
            description="可以切换任务视图、来源或查看其它页；读取任务不会重复执行。"
          />
        )
      )}
      <div className="rss-task-pagination">
        <span>第 {state.page} 页 · 本页搜索不跨未读取的记录</span>
        <div>
          <Button
            disabled={state.loading || !state.previous.length}
            onClick={() => void owner.previousPage()}
          >
            上一页
          </Button>
          <Button
            disabled={state.loading || !state.next}
            onClick={() => void owner.load(true)}
          >
            下一页
          </Button>
        </div>
      </div>
      <p className="rss-task-protocol">
        执行结束表示进程或操作已返回；实际状态符合预期后，才显示核实成功。请求取消不代表已确认终止，也不承诺回滚外部效果。
      </p>
    </div>
  );
}
