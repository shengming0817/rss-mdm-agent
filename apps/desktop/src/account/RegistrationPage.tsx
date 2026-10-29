import { Button, Tag } from "antd";
import { SafetyOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";
import { useDesktop } from "../state/react";
import { createRegistration, registrationLabels } from "./registration";
import "./account.css";
export function RegistrationPage({
  owner,
  onLogin,
}: {
  owner: ReturnType<typeof createRegistration>;
  onLogin(): void;
}) {
  const desktop = useDesktop(),
    state = useSnapshot(owner.state),
    account = useSnapshot(desktop.account),
    application = useSnapshot(desktop.state),
    registered = state.registration;
  const unresolved =
    registered &&
    !["unregistered", "rejected", "paused"].includes(registered.phase);
  const request = async () => {
    try {
      await desktop.service?.requestRegistration();
    } catch {
      /* The desktop owner retains the closed failure after identity replacement. */
    }
  };
  const importConfiguration = async () => {
    try {
      await desktop.service?.importRegistration();
    } catch {
      /* Same identity owner and error projection as registration. */
    }
  };
  const mdmLabel =
    state.mdm?.state === "this_organization"
      ? "由当前组织管理"
      : state.mdm?.state === "other_organization"
        ? "由其他组织管理"
        : state.mdm?.state === "unenrolled"
          ? "尚未加入 MDM"
          : "MDM 状态未知";
  return (
    <div className="rss-page rss-stack">
      <PageHeader
        title="设备注册与 MDM"
        description="分别查看设备注册、凭据与系统管理状态。"
        actions={
          <Button loading={state.loading} onClick={() => void owner.refresh()}>
            重新读取
          </Button>
        }
      />
      {application.error && (
        <StatePanel
          kind="error"
          title="注册尚未确认"
          description={application.error}
        />
      )}
      <SectionCard title="设备注册">
        <div className="rss-account-profile">
          <span className="rss-avatar">
            <SafetyOutlined />
          </span>
          <div className="rss-grow">
            <h2>
              {registered?.registered
                ? "设备已注册"
                : registered
                  ? registrationLabels[registered.phase]
                  : "设备状态未读取"}
            </h2>
            <p className="rss-muted">
              {registered
                ? registrationLabels[registered.phase]
                : state.registrationError || "正在读取本机注册记录。"}
            </p>
          </div>
          <Tag color={registered?.registered ? "green" : "default"}>
            {registered?.registered ? "已登记" : "待核实"}
          </Tag>
        </div>
        {state.registrationError && (
          <StatePanel
            kind="unknown"
            title={state.registrationError}
            onRead={() => void owner.refresh()}
          />
        )}
        {registered && (
          <dl className="rss-account-facts">
            <dt>注册阶段</dt>
            <dd>{registrationLabels[registered.phase]}</dd>
            <dt>凭据状态</dt>
            <dd>
              {registered.credential === "active"
                ? "当前有效"
                : registered.credential === "absent"
                  ? "尚无凭据"
                  : registered.credential === "expired"
                    ? "已过期"
                    : registered.credential === "revoked"
                      ? "已撤销"
                      : "已退休"}
            </dd>
            {registered.expiresAt != null && (
              <>
                <dt>原授权期限</dt>
                <dd>
                  {new Date(registered.expiresAt * 1000).toLocaleString()}
                </dd>
              </>
            )}
          </dl>
        )}
        {registered?.failure && (
          <StatePanel
            kind="unknown"
            title="注册结果尚未确认"
            description="请重新读取原注册状态；未核实前不重复注册。"
            onRead={() => void owner.refresh()}
          />
        )}
        <div className="rss-account-actions">
          {account.user?.identity.mode === "user_device" ? (
            <Button
              type="primary"
              onClick={() => void request()}
              disabled={
                !registered ||
                registered.registered ||
                !!unresolved ||
                application.registrationPending
              }
            >
              注册这台设备
            </Button>
          ) : (
            <Button type="primary" onClick={onLogin}>
              登录企业账号后注册
            </Button>
          )}
          <Button
            onClick={() => void importConfiguration()}
            disabled={application.registrationPending || account.busy}
          >
            导入组织注册配置
          </Button>
        </div>
        <p className="rss-muted">
          注册授权、凭据激活和本机运行状态分别核实。停止等待不会撤销已受理的注册。
        </p>
      </SectionCard>
      <SectionCard title="MDM 管理">
        <h2>{mdmLabel}</h2>
        <p className="rss-muted">
          {state.mdmError ||
            "设备注册不等于系统已加入 MDM；这里显示本机系统观察。"}
        </p>
        {state.mdm?.observedAt != null && (
          <p className="rss-muted">
            观察时间：{new Date(state.mdm.observedAt * 1000).toLocaleString()}
          </p>
        )}
      </SectionCard>
      <SectionCard title="本机服务">
        <h3>
          {state.service?.phase === "connected"
            ? "本机服务已连接"
            : state.service?.phase === "notInstalled"
              ? "尚未安装本机服务"
              : state.service?.phase === "configurationRequired"
                ? "本机服务需要配置"
                : state.service?.phase === "mismatch"
                  ? "本机服务版本不匹配"
                  : state.service?.phase === "rejected"
                    ? "本机服务拒绝连接"
                    : "本机服务未连接"}
        </h3>
        <p className="rss-muted">
          {state.serviceError ||
            (state.service?.phase === "connected"
              ? `版本 ${state.service.status.build} · ${state.service.status.readiness.phase === "ready" ? "运行已就绪" : "尚未就绪"}`
              : "请检查安装与服务配置，然后重新读取。")}
        </p>
      </SectionCard>
    </div>
  );
}
