import { useEffect, useState } from "react";
import { Alert, Button, Input, Modal, Popconfirm, Select, Tag } from "antd";
import { PlusOutlined, RightOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { deepClone } from "valtio/vanilla/utils";
import { PageHeader, StatePanel } from "@rss-mdm-agent/ui";
import type {
  Connection,
  ConnectionDraft,
  ConnectionSource,
} from "@rss-mdm-agent/ai-contract/wire";
import type { Workspace } from "../state/workspace";
import { SessionOptions } from "./SessionOptions";
const providerNames = {
  codex: "Codex",
  claude: "Claude",
  deepseek: "DeepSeek",
};
function endpointLabel(source: ConnectionSource) {
  if (source.type === "existing_config") return "现有官方工具配置";
  try {
    const url = new URL(source.apiUrl);
    return `${url.origin}${url.pathname}`;
  } catch {
    return "自定义 API";
  }
}
export function ConnectionsPage({
  workspace,
  active,
}: {
  workspace: Workspace;
  active: boolean;
}) {
  const assistant = useSnapshot(workspace.assistant.state),
    state = useSnapshot(workspace.connections.state),
    [editing, setEditing] = useState<{
      draft: ConnectionDraft;
      expected: number | null;
    } | null>(null),
    [secret, setSecret] = useState(""),
    [sessionOptions, setSessionOptions] = useState(false),
    [stale, setStale] = useState(false);
  useEffect(() => {
    if (!active) {
      setSecret("");
      setSessionOptions(false);
    }
  }, [active]);
  useEffect(
    () => () => {
      setSecret("");
    },
    [workspace],
  );
  const close = () => {
      setSecret("");
      setEditing(null);
      setStale(false);
    },
    patch = (update: Partial<ConnectionDraft>) => {
      setSecret("");
      setEditing((previous) =>
        previous
          ? { ...previous, draft: { ...previous.draft, ...update } }
          : null,
      );
    },
    edit = (connection: Connection) => {
      setSecret("");
      setStale(false);
      setEditing({
        expected: connection.configRevision,
        draft: {
          connectionId: connection.connectionId,
          provider: connection.provider,
          profile: connection.profile,
          source: deepClone(connection.source),
          presentation: { name: connection.presentation.name },
        },
      });
    },
    draft = editing?.draft,
    source = draft?.source;
  const valid = (() => {
    if (!draft?.presentation.name.trim()) return false;
    if (source?.type !== "custom_api") return true;
    try {
      const url = new URL(source.apiUrl);
      return (
        ["https:", "http:"].includes(url.protocol) &&
        !url.username &&
        !url.password &&
        !url.search &&
        !url.hash &&
        !!source.model.trim()
      );
    } catch {
      return false;
    }
  })();
  return (
    <div className="rss-page rss-preferences-page">
      <p className="rss-eyebrow">PREFERENCES</p>
      <PageHeader
        title="AI 与连接"
        description="每个会话使用所选连接；更改配置不会自动重发已有输入。"
        actions={
          <>
            <Button onClick={() => setSessionOptions(true)}>会话选项</Button>
            <Button
              disabled={state.busy || assistant.connection !== "connected"}
              onClick={() => void workspace.connections.refresh()}
            >
              重读连接
            </Button>
            <Button
              aria-label="添加连接"
              icon={<PlusOutlined />}
              disabled={!workspace.connections.canSave || state.busy}
              onClick={() => {
                setSecret("");
                setEditing({
                  expected: null,
                  draft: {
                    connectionId: crypto.randomUUID(),
                    provider: "codex",
                    profile: "conversation",
                    source: { type: "existing_config" },
                    presentation: { name: "" },
                  },
                });
              }}
            >
              添加连接
            </Button>
          </>
        }
      />
      <div className="rss-stack">
        {state.message && <Alert type="info" title={state.message} />}
        {assistant.connection !== "connected" && (
          <StatePanel
            kind="offline"
            title="AI 控制连接未就绪"
            description="已读取配置可供参考。可重新连接后重读；不会自动测试或发送对话。"
            onRead={() => void workspace.assistant.connect()}
          />
        )}
        {assistant.connections
          .filter((value) => value.status !== "deleted")
          .map((connection) => (
            <article
              className="rss-connection-card"
              key={connection.connectionId}
            >
              <span className="rss-connection-mark">
                {providerNames[connection.provider].slice(0, 1)}
              </span>
              <div className="rss-grow">
                <h2>{connection.presentation.name}</h2>
                <p>
                  {providerNames[connection.provider]} ·{" "}
                  {endpointLabel(connection.source)}
                </p>
                <small>
                  {connection.profile === "controlled_tools"
                    ? "受控工具"
                    : "对话"}{" "}
                  · 配置版本 {connection.configRevision}
                </small>
                {connection.lastTest?.outcome === "failed" && (
                  <small>验证未通过 · {connection.lastTest.stage}</small>
                )}
              </div>
              <Tag color={connection.status === "ready" ? "green" : "gold"}>
                {connection.status === "ready" ? "已验证" : "待验证"}
              </Tag>
              {assistant.preferences?.defaultConnectionId ===
              connection.connectionId ? (
                <Tag>新会话默认</Tag>
              ) : (
                <Button
                  size="small"
                  disabled={
                    state.busy ||
                    assistant.connection !== "connected" ||
                    connection.status !== "ready"
                  }
                  aria-label={`将 ${connection.presentation.name} 设为新会话默认`}
                  onClick={() =>
                    void workspace.connections.setDefault(
                      deepClone(connection) as Connection,
                    )
                  }
                >
                  设为默认
                </Button>
              )}
              <Button
                type="text"
                aria-label={`配置 ${connection.presentation.name}`}
                icon={<RightOutlined />}
                disabled={state.busy}
                onClick={() => edit(deepClone(connection) as Connection)}
              />
            </article>
          ))}
        {!assistant.connections.some((value) => value.status !== "deleted") &&
          assistant.connection === "connected" && (
            <StatePanel
              kind="empty"
              title="还没有 AI 连接"
              description="可添加现有工具配置，或填写自定义 API。"
            />
          )}
        <Alert
          type="info"
          title="已保存的秘密材料不会回显。新密钥只通过本机受保护保存入口提交。"
        />
      </div>
      {active && sessionOptions && (
        <SessionOptions
          workspace={workspace}
          onClose={() => setSessionOptions(false)}
        />
      )}
      <Modal
        open={active && !!editing}
        title={editing?.expected == null ? "添加 AI 连接" : "配置 AI 连接"}
        onCancel={close}
        footer={null}
        destroyOnHidden
        width={620}
      >
        {draft && source && (
          <div className="rss-stack rss-connection-form">
            <label className="rss-field">
              <span>连接名称</span>
              <Input
                aria-label="连接名称"
                maxLength={120}
                value={draft.presentation.name}
                disabled={state.busy}
                onChange={(event) =>
                  setEditing((previous) =>
                    previous
                      ? {
                          ...previous,
                          draft: {
                            ...previous.draft,
                            presentation: { name: event.target.value },
                          },
                        }
                      : null,
                  )
                }
              />
            </label>
            <div className="rss-form-grid">
              <label className="rss-field">
                <span>提供方</span>
                <Select
                  aria-label="AI 提供方"
                  value={draft.provider}
                  disabled={state.busy}
                  options={Object.entries(providerNames).map(
                    ([value, label]) => ({ value, label }),
                  )}
                  onChange={(provider) => patch({ provider })}
                />
              </label>
              <label className="rss-field">
                <span>用途</span>
                <Select
                  aria-label="AI 连接用途"
                  value={draft.profile}
                  disabled={state.busy}
                  options={[
                    { label: "对话", value: "conversation" },
                    { label: "受控工具", value: "controlled_tools" },
                  ]}
                  onChange={(profile) => patch({ profile })}
                />
              </label>
            </div>
            <label className="rss-field">
              <span>配置来源</span>
              <Select
                aria-label="AI 配置来源"
                value={source.type}
                disabled={state.busy}
                options={[
                  { label: "现有官方工具配置", value: "existing_config" },
                  { label: "自定义 API", value: "custom_api" },
                ]}
                onChange={(type) =>
                  patch({
                    source:
                      type === "custom_api"
                        ? {
                            type,
                            apiUrl: "",
                            model: "",
                            credentialType: "api_key",
                          }
                        : { type: "existing_config" },
                  })
                }
              />
            </label>
            {source.type === "existing_config" ? (
              <label className="rss-field">
                <span>配置目录（可选）</span>
                <Input
                  aria-label="AI 配置目录"
                  disabled={state.busy}
                  value={source.directory ?? ""}
                  placeholder="留空使用官方默认目录"
                  onChange={(event) =>
                    patch({
                      source: {
                        ...source,
                        directory: event.target.value || undefined,
                      },
                    })
                  }
                />
              </label>
            ) : (
              <>
                <label className="rss-field">
                  <span>API 地址</span>
                  <Input
                    aria-label="AI API 地址"
                    disabled={state.busy}
                    value={source.apiUrl}
                    placeholder="https://api.example.com"
                    onChange={(event) =>
                      patch({
                        source: { ...source, apiUrl: event.target.value },
                      })
                    }
                  />
                </label>
                <label className="rss-field">
                  <span>模型</span>
                  <Input
                    aria-label="AI 模型"
                    disabled={state.busy}
                    value={source.model}
                    onChange={(event) =>
                      patch({
                        source: { ...source, model: event.target.value },
                      })
                    }
                  />
                </label>
                <label className="rss-field">
                  <span>凭据类型</span>
                  <Select
                    aria-label="AI 凭据类型"
                    disabled={state.busy}
                    value={source.credentialType ?? "api_key"}
                    options={[
                      { label: "API Key", value: "api_key" },
                      { label: "Auth Token", value: "auth_token" },
                    ]}
                    onChange={(credentialType) =>
                      patch({ source: { ...source, credentialType } })
                    }
                  />
                </label>
                <label className="rss-field">
                  <span>
                    {editing?.expected == null
                      ? "新密钥"
                      : "替换密钥（留空保留原密钥）"}
                  </span>
                  <Input.Password
                    aria-label="AI 新密钥"
                    autoComplete="new-password"
                    disabled={state.busy}
                    value={secret}
                    onChange={(event) => setSecret(event.target.value)}
                  />
                </label>
              </>
            )}
            <p className="rss-muted">
              保存核对当前配置版本；验证可能访问提供方服务。密钥不会进入页面上下文、搜索或诊断内容。
            </p>
            {stale && (
              <Alert
                type="warning"
                title="此配置已变化或移除，请关闭并重新打开当前配置后继续。"
              />
            )}
            <div className="rss-settings-actions">
              <Button onClick={close}>关闭</Button>
              <Button
                type="primary"
                disabled={
                  state.busy || !workspace.connections.canSave || !valid
                }
                onClick={() => {
                  const current = editing;
                  if (!current) return;
                  const value = secret;
                  setSecret("");
                  void workspace.connections
                    .save(current.draft, current.expected, value || undefined)
                    .then((saved) => {
                      if (saved) setEditing(null);
                    });
                }}
              >
                保存连接
              </Button>
              {editing?.expected != null && (
                <>
                  <Button
                    disabled={
                      state.busy || assistant.connection !== "connected"
                    }
                    onClick={() => {
                      const connection =
                        workspace.assistant.state.connections.find(
                          (value) =>
                            value.connectionId === draft.connectionId &&
                            value.configRevision === editing.expected,
                        );
                      if (connection) {
                        setSecret("");
                        void workspace.connections
                          .test(deepClone(connection))
                          .then((tested) => {
                            if (tested) close();
                          });
                      } else setStale(true);
                    }}
                  >
                    验证当前已保存配置
                  </Button>
                  <Popconfirm
                    title="移除此连接？"
                    description="现有会话不会自动改用其它连接。"
                    onConfirm={() => {
                      const connection =
                        workspace.assistant.state.connections.find(
                          (value) =>
                            value.connectionId === draft.connectionId &&
                            value.configRevision === editing.expected,
                        );
                      setSecret("");
                      if (connection)
                        void workspace.connections
                          .remove(deepClone(connection))
                          .then((removed) => {
                            if (removed) close();
                          });
                      else setStale(true);
                    }}
                  >
                    <Button
                      danger
                      disabled={
                        state.busy || assistant.connection !== "connected"
                      }
                    >
                      移除连接
                    </Button>
                  </Popconfirm>
                </>
              )}
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
