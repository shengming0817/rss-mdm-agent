import { Button, Card } from "antd";
import { AppstoreOutlined } from "@ant-design/icons";
import type { ReactNode } from "react";

export type CoverTone = "workspace" | "design" | "notes" | "browser" | "system";
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
    <div className={`rss-software-cover rss-cover-${tone}`}>
      <div className="rss-cover-copy">
        <h3>{title}</h3>
        {subtitle && <p>{subtitle}</p>}
      </div>
      <div className="rss-cover-window" aria-hidden="true">
        <div />
        <i />
        <i />
        <i />
        <i />
      </div>
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
