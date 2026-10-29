import { Alert, Button, Card, Drawer, Empty, Skeleton, Tag } from "antd";
import type { ReactNode } from "react";

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="rss-page-heading">
      <div>
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="rss-page-actions">{actions}</div>}
    </header>
  );
}
export function SectionCard({
  title,
  extra,
  children,
  className = "",
}: {
  title?: ReactNode;
  extra?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <Card
      title={title}
      extra={extra}
      className={`rss-section-card ${className}`}
    >
      {children}
    </Card>
  );
}
export type StateKind =
  | "loading"
  | "empty"
  | "error"
  | "offline"
  | "readonly"
  | "unknown";
export function StatePanel({
  kind,
  title,
  description,
  onRead,
  children,
}: {
  kind: StateKind;
  title: string;
  description?: string;
  onRead?(): void;
  children?: ReactNode;
}) {
  if (kind === "loading")
    return (
      <section role="status" aria-label={title} aria-busy="true">
        <Skeleton active paragraph={{ rows: 3 }} />
      </section>
    );
  if (kind === "empty")
    return (
      <Empty
        description={
          <>
            <strong>{title}</strong>
            {description && <p>{description}</p>}
          </>
        }
      >
        {children}
      </Empty>
    );
  return (
    <div className="rss-state-panel">
      <Alert
        showIcon
        type={
          kind === "error" ? "error" : kind === "readonly" ? "info" : "warning"
        }
        title={title}
        description={description}
      />
      {onRead && (
        <Button onClick={onRead}>
          {kind === "unknown" ? "查询原请求" : "重新读取"}
        </Button>
      )}
      {children}
    </div>
  );
}
export type ActionPhase =
  | "received"
  | "running"
  | "ended"
  | "verified"
  | "unknown"
  | "failed"
  | "cancelRequested"
  | "cancelled";
const phases = {
  received: ["已接收", "default"],
  running: ["执行中", "processing"],
  ended: ["执行结束，待核实", "warning"],
  verified: ["核实成功", "success"],
  unknown: ["结果未知", "warning"],
  failed: ["失败", "error"],
  cancelRequested: ["已请求取消", "warning"],
  cancelled: ["已取消", "default"],
} as const;
export function ActionStatus({ phase }: { phase: ActionPhase }) {
  const [label, color] = phases[phase];
  return <Tag color={color}>{label}</Tag>;
}
export function DetailDrawer({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  returnFocus,
}: {
  open: boolean;
  onClose(): void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  returnFocus?(): void;
}) {
  return (
    <Drawer
      open={open}
      onClose={onClose}
      afterOpenChange={(visible) => {
        if (!visible) returnFocus?.();
      }}
      title={
        <>
          <h2>{title}</h2>
          {description && <p className="rss-muted">{description}</p>}
        </>
      }
      size={500}
      footer={footer}
      className="rss-detail-drawer"
      destroyOnHidden
    >
      {children}
    </Drawer>
  );
}
