import { useEffect, useState } from "react";
import { Alert, Button, Form, Input, Modal, Tag } from "antd";
import { UserOutlined, PlusOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";
import { useDesktop } from "../state/react";
import type { Workspace } from "../state/workspace";
import { createOrganizations } from "./organizations";
import type { AccountOrganization } from "@rss-mdm-agent/ai-contract/wire";
import "./account.css";
export function AccountPage({
  workspace,
  onLogin,
}: {
  workspace: Workspace;
  onLogin(): void;
}) {
  const desktop = useDesktop();
  return (
    <div className="rss-page rss-stack">
      <PageHeader title="账号与组织" description="管理当前账号与组织连接。" />
      <SectionCard>
        <div className="rss-account-profile">
          <span className="rss-avatar">
            <UserOutlined />
          </span>
          <div className="rss-grow">
            <h2>{workspace.identity.user.displayName}</h2>
            <p className="rss-muted">
              {workspace.identity.identity.mode === "user_device"
                ? "企业账号 · 当前用户与设备"
                : "当前使用本机工作区"}
            </p>
          </div>
          <Button type="primary" onClick={onLogin}>
            {workspace.identity.identity.mode === "user_device"
              ? "切换账号"
              : "登录企业账号"}
          </Button>
        </div>
        <p className="rss-muted">
          登录与组织权限分别核验。操作仍由本机服务按当前身份确认。
        </p>
      </SectionCard>
      <BindingNotice />
      <OrganizationManager />
      <SectionCard title="本机会话">
        <p className="rss-muted">
          退出企业账号后回到本机工作区。已受理的设备任务仍按原记录查询。
        </p>
        <div className="rss-account-actions">
          <Button onClick={() => void desktop.logout()}>
            {workspace.identity.identity.mode === "user_device"
              ? "退出企业账号"
              : "重新读取本机会话"}
          </Button>
          <Button
            onClick={() =>
              workspace.navigation.select("settings", "registration")
            }
          >
            设备注册与 MDM
          </Button>
        </div>
      </SectionCard>
    </div>
  );
}
export function OrganizationManager() {
  const desktop = useDesktop(),
    [owner, setOwner] = useState<ReturnType<typeof createOrganizations> | null>(
      null,
    );
  useEffect(() => {
    const fixture = desktop.fixture?.account;
    const current = createOrganizations(
      fixture
        ? {
            load: async () => ({
              schemaVersion: 1,
              kind: "accountSettings",
              organizations: fixture.organizations,
              selected: fixture.organizations[0]?.id,
            }),
            save: async () => {
              throw Error("readonly");
            },
          }
        : undefined,
    );
    setOwner(current);
    void current.load();
    return () => current.dispose();
  }, [desktop]);
  return owner ? (
    <OrganizationList owner={owner} />
  ) : (
    <StatePanel kind="loading" title="正在读取组织连接" />
  );
}
export function BindingNotice() {
  const desktop = useDesktop(),
    account = useSnapshot(desktop.account),
    state = useSnapshot(desktop.state);
  const recovery =
    account.binding === "cleanup_pending" ||
    account.binding === "conflict" ||
    account.binding === "invalidated";
  return account.notice || state.error || recovery ? (
    <Alert
      type="warning"
      showIcon
      title={
        state.error ||
        account.notice ||
        "账号状态尚未确认，请先恢复或清理原绑定。"
      }
      action={
        recovery ? (
          <div className="rss-account-actions">
            <Button
              size="small"
              loading={account.busy}
              onClick={() => void desktop.recover()}
            >
              重试原绑定
            </Button>
            <Button
              size="small"
              danger
              onClick={() => void desktop.recover(true)}
              disabled={account.busy}
            >
              清理未确认绑定
            </Button>
          </div>
        ) : undefined
      }
    />
  ) : null;
}
function OrganizationList({
  owner,
}: {
  owner: ReturnType<typeof createOrganizations>;
}) {
  const desktop = useDesktop(),
    state = useSnapshot(owner.state),
    [editing, setEditing] = useState<AccountOrganization | null>(null),
    [form] = Form.useForm<AccountOrganization>();
  const edit = (value?: AccountOrganization) => {
    const record = value ?? {
      id: crypto.randomUUID(),
      label: "",
      origin: "https://",
      tenantId: "",
    };
    form.setFieldsValue(record);
    setEditing(record);
  };
  const save = async () => {
    try {
      const values = await form.validateFields();
      if (await owner.save({ ...editing!, ...values })) setEditing(null);
    } catch {
      /* Form owns field validation feedback. */
    }
  };
  return (
    <SectionCard
      title="组织连接"
      extra={
        <Button
          icon={<PlusOutlined />}
          disabled={!!desktop.fixture}
          onClick={() => edit()}
        >
          添加组织
        </Button>
      }
    >
      {state.error && (
        <StatePanel
          kind="error"
          title="组织连接读取失败"
          description={state.error}
          onRead={() => void owner.load()}
        />
      )}
      {state.loading ? (
        <StatePanel kind="loading" title="正在读取组织连接" />
      ) : state.items.length ? (
        <div className="rss-organizations">
          {state.items.map((item) => (
            <div className="rss-organization-row" key={item.id}>
              <span className="rss-avatar">{item.label.slice(0, 1)}</span>
              <div className="rss-grow">
                <strong>{item.label}</strong>
                {state.selected === item.id && (
                  <Tag color="purple">当前连接</Tag>
                )}
                <p className="rss-muted">{item.origin}</p>
              </div>
              <Button
                disabled={!!desktop.fixture}
                onClick={() => edit({ ...item })}
              >
                编辑
              </Button>
            </div>
          ))}
        </div>
      ) : (
        <StatePanel
          kind="empty"
          title="还没有组织连接"
          description="添加组织提供的服务地址与租户标识，再登录企业账号。"
        />
      )}
      <Modal
        open={!!editing}
        title="组织连接"
        onCancel={() => {
          setEditing(null);
          form.resetFields();
        }}
        onOk={() => void save()}
        confirmLoading={state.saving}
        destroyOnHidden
        okText="保存连接"
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="label"
            label="组织名称"
            rules={[{ required: true, message: "请输入组织名称" }]}
          >
            <Input aria-label="组织名称" maxLength={128} />
          </Form.Item>
          <Form.Item
            name="origin"
            label="服务地址"
            rules={[
              { required: true, message: "请输入组织提供的 HTTPS 地址" },
              { type: "url", message: "请输入完整服务地址" },
            ]}
          >
            <Input
              aria-label="组织服务地址"
              placeholder="https://mdm.example.com"
              maxLength={2048}
            />
          </Form.Item>
          <Form.Item
            name="tenantId"
            label="租户 UUID"
            rules={[{ required: true, message: "请输入组织提供的租户 UUID" }]}
          >
            <Input aria-label="组织租户 UUID" maxLength={64} />
          </Form.Item>
        </Form>
        {state.error && <Alert type="error" title={state.error} />}
      </Modal>
    </SectionCard>
  );
}
