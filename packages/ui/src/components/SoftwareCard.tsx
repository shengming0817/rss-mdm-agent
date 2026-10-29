import { Button, Card } from "antd";
import { AppstoreOutlined } from "@ant-design/icons";
import type { ReactNode } from "react";

export type CoverTone =
  | "workspace"
  | "design"
  | "notes"
  | "browser"
  | "pdf"
  | "sync"
  | "system";
/** Locally drawn artwork: company content never injects HTML or external image URLs. */
export function SoftwareCover({
  title,
  subtitle,
  tone,
}: {
  title: string;
  subtitle?: string;
  tone: CoverTone;
}) {
  return (
    <div className={`rss-store-art rss-cover-${tone}`} aria-hidden="true">
      {tone === "notes" || tone === "pdf" ? (
        <>
          <div className="rss-paper">
            <span>{title}</span>
            <i />
            <i />
            <i />
            <i />
          </div>
          {subtitle && <small>{subtitle}</small>}
        </>
      ) : tone === "design" ? (
        <>
          <div className="rss-design-board">
            <i />
            <span>Aa</span>
          </div>
          <div className="rss-swatches">
            <i />
            <i />
            <i />
          </div>
        </>
      ) : tone === "browser" ? (
        <div className="rss-browser-window">
          <header>
            ••• <i />
          </header>
          <div>
            <span />
            <i />
          </div>
        </div>
      ) : (
        <>
          <div className="rss-workspace-window">
            <div className="rss-art-side">
              <i />
              <i />
              <i />
              <i />
            </div>
            <div className="rss-art-main">
              <i />
              <i />
              <i />
              <i />
            </div>
          </div>
          {subtitle && <small>{subtitle}</small>}
        </>
      )}
    </div>
  );
}

export function SoftwareCard({
  title,
  description,
  cover,
  icon,
  onDetails,
  action,
}: {
  title: string;
  description: string;
  cover: ReactNode;
  icon?: ReactNode;
  onDetails(): void;
  action?: ReactNode;
}) {
  return (
    <Card
      className="rss-software-card"
      cover={
        <button
          type="button"
          className="rss-artwork-link"
          aria-label={`查看 ${title}`}
          onClick={onDetails}
        >
          {cover}
        </button>
      }
    >
      <div className="rss-software-card-footer">
        <span className="rss-app-icon" aria-hidden="true">
          {icon ?? <AppstoreOutlined />}
        </span>
        <div className="rss-grow">
          <Button type="text" onClick={onDetails} className="rss-app-name">
            {title}
          </Button>
          <p>{description}</p>
        </div>
        {action}
      </div>
    </Card>
  );
}
