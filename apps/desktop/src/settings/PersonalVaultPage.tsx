import { useEffect, useState } from "react";
import { Alert, Button, Input, Modal, Popconfirm, Tag } from "antd";
import { LockOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { usePlatform } from "../state/react";
export function PersonalVaultPage({
  workspace,
  active,
}: {
  workspace: Workspace;
  active: boolean;
}) {
  const owner = workspace.vault,
    state = useSnapshot(owner.state),
    value = state.value,
    platform = usePlatform(),
    [reset, setReset] = useState(false),
    [confirmation, setConfirmation] = useState("");
  useEffect(() => {
    if (active) void owner.run({ kind: "status" });
    else {
      setReset(false);
      setConfirmation("");
    }
  }, [owner, active]);
  return (
    <div className="rss-page rss-preferences-page">
      <p className="rss-eyebrow">PREFERENCES</p>
      <PageHeader
        title="个人 Passkey"
        description="由当前系统用户的保护能力保存，独立于企业账号和 AI 连接。"
        actions={
          <Button
            disabled={!owner.available || state.busy}
            onClick={() => void owner.run({ kind: "status" })}
          >
            重读状态
          </Button>
        }
      />
      <div className="rss-stack">
        {!owner.available ? (
          <StatePanel kind="readonly" title="当前装配未提供个人 Passkey 能力" />
        ) : state.busy && !value ? (
          <StatePanel kind="loading" title="正在读取个人 Vault" />
        ) : null}
        {state.error && <Alert type="warning" title={state.error} />}
        {value && (
          <>
            <SectionCard
              title={
                <>
                  <LockOutlined /> 本机个人 Vault
                </>
              }
              extra={
                <Tag>
                  {!value.initialized
                    ? "尚未创建"
                    : value.unlocked
                      ? "已解锁"
                      : "已锁定"}
                </Tag>
              }
            >
              <dl className="rss-account-facts">
                <dt>归属</dt>
                <dd>当前系统用户</dd>
                <dt>口令解锁</dt>
                <dd>{value.passwordEnabled ? "已启用" : "未启用"}</dd>
                <dt>系统提供方</dt>
                <dd>
                  {value.provider?.enabled === true
                    ? "已启用"
                    : value.provider?.enabled === false
                      ? "未启用"
                      : "状态未知"}
                </dd>
                <dt>索引</dt>
                <dd>
                  {value.provider?.indexPending
                    ? "需要更新"
                    : value.provider
                      ? "无待处理更新"
                      : "尚未取得"}
                </dd>
              </dl>
              <div className="rss-settings-actions">
                {!value.initialized ? (
                  <Button
                    type="primary"
                    disabled={state.busy}
                    onClick={() => void owner.run({ kind: "create" })}
                  >
                    创建个人 Vault
                  </Button>
                ) : (
                  <>
                    {!value.unlocked ? (
                      <>
                        <Button
                          disabled={state.busy}
                          onClick={() =>
                            void owner.run({ kind: "unlock", method: "system" })
                          }
                        >
                          通过系统验证解锁
                        </Button>
                        {value.passwordEnabled && (
                          <Button
                            disabled={state.busy}
                            onClick={() =>
                              void owner.run({
                                kind: "unlock",
                                method: "password",
                              })
                            }
                          >
                            通过原生口令对话框解锁
                          </Button>
                        )}
                      </>
                    ) : (
                      <Button
                        disabled={state.busy}
                        onClick={() => void owner.run({ kind: "lock" })}
                      >
                        锁定
                      </Button>
                    )}
                    <Button
                      disabled={state.busy}
                      onClick={() =>
                        void owner.run({
                          kind: "set_password",
                          authorization: "system",
                        })
                      }
                    >
                      {value.passwordEnabled ? "修改个人口令" : "设置个人口令"}
                    </Button>
                    {value.passwordEnabled && (
                      <>
                        <Button
                          disabled={state.busy}
                          onClick={() =>
                            void owner.run({
                              kind: "set_password",
                              authorization: "password",
                            })
                          }
                        >
                          使用当前口令修改
                        </Button>
                        <Button
                          disabled={state.busy}
                          onClick={() =>
                            void owner.run({ kind: "repair_system" })
                          }
                        >
                          修复系统解锁
                        </Button>
                        <Popconfirm
                          title="移除个人口令？"
                          description="需要系统验证；不会修改网站 Passkey。"
                          onConfirm={() =>
                            void owner.run({ kind: "remove_password" })
                          }
                        >
                          <Button disabled={state.busy}>移除个人口令</Button>
                        </Popconfirm>
                      </>
                    )}
                    {platform === "macos" && (
                      <Button
                        disabled={state.busy}
                        onClick={() => void owner.run({ kind: "repair_index" })}
                      >
                        更新系统索引
                      </Button>
                    )}
                  </>
                )}
              </div>
            </SectionCard>
            {value.initialized && (
              <SectionCard title="网站 Passkey">
                {value.unlocked ? (
                  value.entries.length ? (
                    value.entries.map((entry, index) => (
                      <div
                        className="rss-vault-entry"
                        key={`${entry.rpId}:${index}`}
                      >
                        <span className="rss-grow">
                          <strong>{entry.rpId}</strong>
                          <small>
                            {entry.displayName || entry.username} ·{" "}
                            {entry.backedUp ? "已标记备份" : "未标记备份"}
                          </small>
                        </span>
                        <Popconfirm
                          title={`删除 ${entry.rpId} 的本机 Passkey？`}
                          description="只删除此次观察版本中的原条目；不会撤销网站账号。"
                          onConfirm={() =>
                            void owner.run({
                              kind: "delete",
                              credential_id: [...entry.credentialId],
                              revision: value.revision,
                            })
                          }
                        >
                          <Button danger disabled={state.busy}>
                            删除
                          </Button>
                        </Popconfirm>
                      </div>
                    ))
                  ) : (
                    <StatePanel kind="empty" title="还没有网站 Passkey" />
                  )
                ) : (
                  <StatePanel kind="readonly" title="解锁后查看网站条目" />
                )}
                <p className="rss-muted">
                  仅显示网站与账号元信息；凭据标识、用户句柄和私钥不在页面中展示。
                </p>
              </SectionCard>
            )}
            {value.initialized && (
              <SectionCard title="重置本机 Vault">
                <p className="rss-muted">
                  删除此系统用户的本机 Vault
                  内容。网站账号和组织设备注册不受此操作管理。
                </p>
                <Button
                  danger
                  disabled={state.busy}
                  onClick={() => {
                    setReset(true);
                    setConfirmation("");
                  }}
                >
                  重置本机 Vault
                </Button>
              </SectionCard>
            )}
          </>
        )}
        <Alert
          type="info"
          title="密码输入与用户验证只在受保护的原生对话框中进行。"
        />
      </div>
      <Modal
        open={active && reset}
        title="重置本机个人 Vault"
        onCancel={() => {
          setReset(false);
          setConfirmation("");
        }}
        onOk={() => {
          const text = confirmation;
          setConfirmation("");
          setReset(false);
          void owner.run({ kind: "reset", confirmation: text });
        }}
        okText="确认重置"
        okButtonProps={{
          danger: true,
          disabled: confirmation !== "DELETE LOCAL VAULT" || state.busy,
        }}
      >
        <p>此操作无法恢复本机条目。请输入 DELETE LOCAL VAULT 确认。</p>
        <Input
          aria-label="Vault 重置确认文字"
          value={confirmation}
          onChange={(event) => setConfirmation(event.target.value)}
        />
      </Modal>
    </div>
  );
}
