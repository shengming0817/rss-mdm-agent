import { useEffect, useState } from "react";
import { Alert, Button, Input, InputNumber, Select, Switch, Tag } from "antd";
import { useSnapshot } from "valtio";
import { DetailDrawer, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { boundedText, policyPresentation } from "../software/presentation";

export function PolicyDrawer({
  workspace,
  open,
  onClose,
}: {
  workspace: Workspace;
  open: boolean;
  onClose(): void;
}) {
  const owner = workspace.policies,
    state = useSnapshot(owner.state),
    item = state.item,
    [parameters, setParameters] = useState<Record<string, unknown>>({});
  useEffect(() => {
    const values: Record<string, unknown> = {};
    if (
      item?.task.kind === "script" &&
      typeof item.task.parameterSchema === "object"
    ) {
      const raw = item.task.parameterSchema.properties;
      if (raw && typeof raw === "object" && !Array.isArray(raw)) {
        for (const [name, source] of Object.entries(
          item.task.parameterSources,
        )) {
          const field = (raw as Record<string, unknown>)[name];
          if (
            source.kind === "input" &&
            field &&
            typeof field === "object" &&
            "type" in field &&
            field.type === "boolean"
          )
            values[name] = false;
        }
      }
    }
    setParameters(open ? values : {});
  }, [open, item?.policyId]);
  const schema =
      item?.task.kind === "script" &&
      typeof item.task.parameterSchema === "object"
        ? item.task.parameterSchema
        : {},
    properties =
      schema.properties &&
      typeof schema.properties === "object" &&
      !Array.isArray(schema.properties)
        ? (schema.properties as Record<string, unknown>)
        : {},
    required = Array.isArray(schema.required) ? schema.required : [],
    inputs =
      item?.task.kind === "script"
        ? Object.entries(item.task.parameterSources)
            .filter(([, value]) => value.kind === "input")
            .map(([name]) => {
              const raw = properties[name],
                field =
                  raw && typeof raw === "object" && !Array.isArray(raw)
                    ? (raw as Record<string, unknown>)
                    : {};
              return {
                name,
                field,
                supported:
                  ["string", "number", "integer", "boolean"].includes(
                    String(field.type),
                  ) &&
                  field.format !== "password" &&
                  field.writeOnly !== true,
              };
            })
        : [],
    missing = inputs.some(
      ({ name, supported }) =>
        !supported ||
        (required.includes(name) &&
          (parameters[name] === undefined || parameters[name] === "")),
    ),
    preparation =
      state.status?.kind === "preparation" ? state.status.value : null,
    presentation = item ? policyPresentation(item) : null;
  const update = (name: string, value: unknown) =>
    setParameters((previous) => ({ ...previous, [name]: value }));
  return (
    <DetailDrawer
      open={open}
      onClose={onClose}
      title={presentation?.name ?? "准备组织项目"}
      description="核对当前发布内容；提交后由原服务重新检查授权和参数。"
    >
      <div className="rss-stack">
        {state.busy && (
          <StatePanel kind="loading" title="正在读取原项目或请求" />
        )}
        {state.error && <Alert type="warning" title={state.error} />}
        {item && (
          <>
            <p>{presentation?.description}</p>
            <dl className="rss-account-facts">
              <dt>来源</dt>
              <dd>当前主体可见的组织发布目录</dd>
              <dt>任务类型</dt>
              <dd>{item.task.kind}</dd>
              <dt>发布版本</dt>
              <dd className="rss-reference">{item.policyVersion}</dd>
              <dt>作用主体</dt>
              <dd>
                {item.subject.kind === "device" ? "设备" : "当前企业用户"}
              </dd>
              <dt>执行身份</dt>
              <dd>
                {item.task.kind === "enrollment"
                  ? "设备注册流程"
                  : item.task.runAs}
              </dd>
            </dl>
            {!state.choice &&
              inputs.map(({ name, field, supported }) => (
                <label className="rss-field" key={name}>
                  <span>
                    {boundedText(field.title, 100) || name}
                    {required.includes(name) ? " *" : ""}
                  </span>
                  {!supported ? (
                    <StatePanel
                      kind="readonly"
                      title="此参数需要受保护或复杂输入，当前页面不支持提交。"
                    />
                  ) : Array.isArray(field.enum) &&
                    field.enum.every((value) =>
                      ["string", "number", "boolean"].includes(typeof value),
                    ) ? (
                    <Select
                      aria-label={name}
                      value={
                        parameters[name] === undefined
                          ? undefined
                          : String(field.enum.indexOf(parameters[name]))
                      }
                      options={field.enum.map((value, index) => ({
                        label: String(value),
                        value: String(index),
                      }))}
                      onChange={(index) =>
                        update(name, (field.enum as unknown[])[Number(index)])
                      }
                    />
                  ) : field.type === "boolean" ? (
                    <Switch
                      aria-label={name}
                      checked={parameters[name] === true}
                      onChange={(value) => update(name, value)}
                    />
                  ) : field.type === "number" || field.type === "integer" ? (
                    <InputNumber
                      aria-label={name}
                      value={
                        typeof parameters[name] === "number"
                          ? (parameters[name] as number)
                          : null
                      }
                      precision={field.type === "integer" ? 0 : undefined}
                      onChange={(value) => update(name, value ?? undefined)}
                    />
                  ) : (
                    <Input
                      aria-label={name}
                      maxLength={4096}
                      value={
                        typeof parameters[name] === "string"
                          ? parameters[name]
                          : ""
                      }
                      onChange={(event) => update(name, event.target.value)}
                    />
                  )}
                  {boundedText(field.description) && (
                    <small className="rss-muted">
                      {boundedText(field.description)}
                    </small>
                  )}
                </label>
              ))}
            {preparation && (
              <Tag>
                {preparation.state === "awaitingConfirmation"
                  ? "等待确认"
                  : preparation.state === "failed"
                    ? "原准备失败"
                    : preparation.state === "cancelled"
                      ? "已取消"
                      : "已记录原请求"}
              </Tag>
            )}
            {state.status?.kind === "run" && (
              <Alert
                type="info"
                title="已读取原执行记录，可在任务中心查看进度与核实结果。"
              />
            )}
            {state.choice ? (
              <>
                <p className="rss-reference">原请求：{state.choice.request}</p>
                <Button
                  disabled={state.busy}
                  onClick={() => void owner.recover()}
                >
                  查询原请求
                </Button>
                {state.status?.kind === "absent" && (
                  <Button
                    disabled={state.busy}
                    onClick={() => void owner.submit({})}
                  >
                    重送原请求与原参数
                  </Button>
                )}
                {preparation?.state === "awaitingConfirmation" && (
                  <Button
                    type="primary"
                    disabled={state.busy}
                    onClick={() => void owner.confirm()}
                  >
                    确认原准备
                  </Button>
                )}
                <Button
                  onClick={() => {
                    const request = owner.state.choice?.request;
                    if (request) {
                      onClose();
                      workspace.navigation.select("tasks", "history");
                      void workspace.tasks.openTask({
                        kind: "request",
                        request,
                      });
                    }
                  }}
                >
                  查看原任务
                </Button>
              </>
            ) : (
              <Button
                type="primary"
                disabled={!owner.available || state.busy || missing}
                onClick={() => {
                  const input = { ...parameters };
                  setParameters({});
                  void owner.submit(input);
                }}
              >
                确认并提交
              </Button>
            )}
            <p className="rss-muted">
              请求受理、执行结束和效果核实分别记录。目录可见不代表已获执行许可。
            </p>
          </>
        )}
      </div>
    </DetailDrawer>
  );
}
