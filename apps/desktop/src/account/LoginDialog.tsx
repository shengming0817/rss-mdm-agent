import { useEffect, useRef, useState } from "react";
import {
  Alert,
  Button,
  Form,
  Input,
  Modal,
  QRCode,
  Select,
  Tabs,
  Tag,
} from "antd";
import { LockOutlined, QrcodeOutlined, UserOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { StatePanel } from "@rss-mdm-agent/ui";
import { useDesktop } from "../state/react";
import { createOrganizations } from "./organizations";
import type { FixtureLoginPort } from "./types";
export function LoginDialog({
  open,
  onClose,
  onManage,
}: {
  open: boolean;
  onClose(): void;
  onManage(): void;
}) {
  const desktop = useDesktop(),
    account = useSnapshot(desktop.account),
    clear = useRef(() => {}),
    [organizations, setOrganizations] = useState<ReturnType<
      typeof createOrganizations
    > | null>(null);
  useEffect(() => {
    if (!open) return;
    const fixture = desktop.fixture?.account;
    const owner = createOrganizations(
      fixture
        ? {
            load: async () => ({
              schemaVersion: 1,
              kind: "accountSettings",
              organizations: fixture.organizations,
              selected: fixture.organizations[0]?.id,
            }),
            save: async () => {
              throw Error("fixture organization readonly");
            },
          }
        : undefined,
    );
    setOrganizations(owner);
    void owner.load();
    return () => {
      clear.current();
      owner.dispose();
      fixture?.cancel();
    };
  }, [open, desktop]);
  const close = () => {
    if (desktop.account.busy) return;
    clear.current();
    desktop.fixture?.account?.cancel();
    onClose();
  };
  return (
    <Modal
      open={open}
      title="登录企业账号"
      centered
      footer={null}
      width={450}
      onCancel={close}
      closable={!account.busy}
      keyboard={!account.busy}
      maskClosable={!account.busy}
      destroyOnHidden
    >
      <p className="rss-muted">选择你的组织，连接工作所需的应用与服务。</p>
      {organizations ? (
        <LoginForm
          organizations={organizations}
          clear={clear}
          onClose={close}
          onManage={() => {
            close();
            onManage();
          }}
        />
      ) : (
        <StatePanel kind="loading" title="正在读取组织" />
      )}
    </Modal>
  );
}
function LoginForm({
  organizations,
  clear,
  onClose,
  onManage,
}: {
  organizations: ReturnType<typeof createOrganizations>;
  clear: { current: () => void };
  onClose(): void;
  onManage(): void;
}) {
  const desktop = useDesktop(),
    state = useSnapshot(organizations.state),
    account = useSnapshot(desktop.account),
    desktopState = useSnapshot(desktop.state),
    [organization, setOrganization] = useState(""),
    [login, setLogin] = useState(""),
    [password, setPassword] = useState(""),
    [method, setMethod] = useState("password");
  const selected = organization || state.selected;
  useEffect(() => {
    clear.current = () => setPassword("");
    return () => {
      clear.current = () => {};
    };
  }, [clear]);
  async function submit() {
    if (!selected || !login.trim() || !password || account.busy) return;
    const secret = password;
    setPassword("");
    if (
      await desktop.login({
        organizationId: selected,
        login: login.trim(),
        password: secret,
      })
    )
      onClose();
  }
  const fixture = desktop.fixture?.account;
  return (
    <div className="rss-account-form">
      {state.error && (
        <StatePanel
          kind="error"
          title="组织读取失败"
          description={state.error}
          onRead={() => void organizations.load()}
        />
      )}
      {desktopState.error && (
        <Alert type="error" showIcon title={desktopState.error} />
      )}
      <Form layout="vertical" onFinish={() => void submit()}>
        <Form.Item label="组织">
          <Select
            aria-label="组织"
            loading={state.loading}
            disabled={account.busy}
            value={selected || undefined}
            placeholder="选择组织"
            options={state.items.map((item) => ({
              value: item.id,
              label: item.label,
            }))}
            onChange={(value) => {
              setPassword("");
              setOrganization(value);
              fixture?.organization(value);
            }}
          />
        </Form.Item>
        <Tabs
          activeKey={method}
          onChange={(value) => {
            if (account.busy) return;
            setPassword("");
            fixture?.cancel();
            setMethod(value);
            if (value === "qr") fixture?.refreshQr();
            if (value === "client") void fixture?.discover();
          }}
          items={[
            {
              key: "password",
              label: "账号密码",
              children: (
                <>
                  <Form.Item label="账号">
                    <Input
                      prefix={<UserOutlined />}
                      aria-label="企业账号"
                      autoComplete="username"
                      value={login}
                      disabled={account.busy}
                      onChange={(event) => setLogin(event.target.value)}
                      maxLength={256}
                    />
                  </Form.Item>
                  <Form.Item label="密码">
                    <Input.Password
                      prefix={<LockOutlined />}
                      aria-label="企业密码"
                      autoComplete="current-password"
                      value={password}
                      disabled={account.busy}
                      onChange={(event) => setPassword(event.target.value)}
                      maxLength={4096}
                    />
                  </Form.Item>
                  <Button
                    block
                    type="primary"
                    htmlType="submit"
                    loading={account.busy}
                    disabled={!selected || !login.trim() || !password}
                  >
                    登录
                  </Button>
                </>
              ),
            },
            {
              key: "qr",
              label: "扫码登录",
              children: fixture ? (
                <QrLogin port={fixture} onSuccess={onClose} />
              ) : (
                <StatePanel
                  kind="readonly"
                  title="当前组织暂不提供扫码登录"
                  description="可以使用账号密码登录。"
                />
              ),
            },
            {
              key: "client",
              label: "应用登录",
              children: fixture ? (
                <ClientLogin port={fixture} onSuccess={onClose} />
              ) : (
                <StatePanel
                  kind="readonly"
                  title="当前连接暂不提供应用登录"
                  description="可以使用账号密码登录。"
                />
              ),
            },
          ]}
        />
      </Form>
      <div className="rss-account-form-footer">
        <Button type="link" onClick={onManage} disabled={account.busy}>
          管理组织连接
        </Button>
        <Button type="text" onClick={onClose} disabled={account.busy}>
          暂不登录
        </Button>
      </div>
    </div>
  );
}
function QrLogin({
  port,
  onSuccess,
}: {
  port: FixtureLoginPort;
  onSuccess(): void;
}) {
  const desktop = useDesktop(),
    state = useSnapshot(port.state),
    account = useSnapshot(desktop.account);
  const labels = {
    waiting: "请扫码后在应用中确认",
    scanned: "已扫描，请确认本次登录",
    expired: "二维码已过期",
    denied: "本次登录未获确认",
  };
  return (
    <div className="rss-qr-login">
      <Tag>显式演示登录</Tag>
      <QRCode
        value="rss-fixture://login/isolated-example"
        status={
          state.qr === "expired" || state.qr === "denied" ? "expired" : "active"
        }
        onRefresh={() => port.refreshQr()}
        iconSize={0}
      />
      <p>{labels[state.qr]}</p>
      {state.error && <Alert type="warning" title={state.error} />}
      {state.qr === "scanned" ? (
        <Button
          type="primary"
          loading={account.busy}
          icon={<QrcodeOutlined />}
          onClick={() =>
            void desktop.confirmQr?.().then((success) => {
              if (success) onSuccess();
            })
          }
        >
          确认本次登录
        </Button>
      ) : (
        <Button onClick={() => port.refreshQr()} disabled={account.busy}>
          刷新二维码
        </Button>
      )}
    </div>
  );
}
function ClientLogin({
  port,
  onSuccess,
}: {
  port: FixtureLoginPort;
  onSuccess(): void;
}) {
  const desktop = useDesktop(),
    state = useSnapshot(port.state),
    account = useSnapshot(desktop.account);
  return (
    <div className="rss-stack">
      <p className="rss-muted">选择已登录的应用来确认本次组织账号。</p>
      {state.error && <Alert type="warning" title={state.error} />}
      {state.discovering ? (
        <StatePanel kind="loading" title="正在读取应用登录状态" />
      ) : state.clients.length ? (
        state.clients.map((client) => (
          <div className="rss-client-login-row" key={client.id}>
            <span className="rss-avatar">
              <UserOutlined />
            </span>
            <div className="rss-grow">
              <strong>{client.name}</strong>
              <p className="rss-muted">
                {client.account || "尚未登录"} ·{" "}
                {client.status === "signed_in"
                  ? "已登录"
                  : client.status === "signed_out"
                    ? "未登录"
                    : client.status === "expired"
                      ? "登录已过期"
                      : "组织不匹配"}
              </p>
            </div>
            <Button
              disabled={client.status !== "signed_in" || account.busy}
              onClick={() =>
                void desktop.loginClient?.(client.id).then((success) => {
                  if (success) onSuccess();
                })
              }
            >
              使用此账号
            </Button>
          </div>
        ))
      ) : (
        <StatePanel
          kind="empty"
          title="没有找到可用的登录应用"
          description="可重新查找，或使用账号密码登录。"
        />
      )}
      <Button
        onClick={() => void port.discover()}
        disabled={account.busy}
        loading={state.discovering}
      >
        重新查找
      </Button>
    </div>
  );
}
