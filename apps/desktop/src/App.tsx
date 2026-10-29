import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";

/** EPIC base only. Business pages are connected by the following implementation issues. */
export function App() {
  return (
    <main className="rss-page">
      <PageHeader title="RSS Agent" description="桌面自助服务" />
      <SectionCard>
        <StatePanel
          kind="readonly"
          title="桌面界面重建中"
          description="当前为工程基础版本。业务页面和服务状态尚未接入。"
        />
      </SectionCard>
    </main>
  );
}
