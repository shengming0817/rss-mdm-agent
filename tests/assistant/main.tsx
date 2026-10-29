import { DesktopFixture } from "./desktop";
import { StateFixture } from "./state";
import { createRoot } from "react-dom/client";
import { useRef, useState } from "react";
import { Button, Dropdown, Form, Input, Select, Switch, Table } from "antd";
import {
  ActionStatus,
  DetailDrawer,
  MessageComposer,
  MessageStream,
  PageHeader,
  SectionCard,
  SoftwareCard,
  SoftwareCover,
  StatePanel,
  UiProvider,
  type ThemePreference,
} from "@rss-mdm-agent/ui";
import "@rss-mdm-agent/ui/style.css";

/** Explicit browser fixture only. No fixture content is reachable in production builds. */
export function FixtureApp() {
  if (new URLSearchParams(window.location.search).has("desktop"))
    return <DesktopFixture />;
  return new URLSearchParams(window.location.search).has("state") ? (
    <StateFixture />
  ) : (
    <FoundationFixture />
  );
}
function FoundationFixture() {
  const [theme, setTheme] = useState<ThemePreference>("system");
  const [open, setOpen] = useState(false);
  const menuTrigger = useRef<HTMLButtonElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const openDetails = () => {
    opener.current = document.activeElement as HTMLElement;
    setOpen(true);
  };
  const [draft, setDraft] = useState("");
  const [sent, setSent] = useState<string[]>([]);
  const [reads, setReads] = useState(0);
  const [notice, setNotice] = useState("");
  return (
    <UiProvider theme={theme}>
      <main className="rss-page rss-stack">
        <PageHeader
          title="组件基础验收"
          description="显式开发装配 · 不代表业务接线或平台验收"
          actions={
            <Select
              aria-label="主题"
              value={theme}
              onChange={setTheme}
              options={[
                { value: "system", label: "跟随系统" },
                { value: "light", label: "浅色" },
                { value: "dark", label: "深色" },
              ]}
            />
          }
        />
        <SectionCard
          title="控件与反馈"
          extra={
            <Dropdown
              menu={{
                items: [
                  {
                    key: "details",
                    label: "查看详情",
                    onClick: () => {
                      opener.current = menuTrigger.current;
                      setOpen(true);
                    },
                  },
                ],
              }}
              trigger={["click"]}
            >
              <Button ref={menuTrigger}>更多操作</Button>
            </Dropdown>
          }
        >
          <Form onFinish={() => setNotice("表单已校验")} layout="vertical">
            <Form.Item
              label="名称"
              name="name"
              rules={[{ required: true, message: "请填写名称" }]}
            >
              <Input />
            </Form.Item>
            <Form.Item label="选项" name="option">
              <Select
                options={[
                  { value: "one", label: "选项一" },
                  { value: "two", label: "选项二" },
                ]}
              />
            </Form.Item>
            <Form.Item label="启用" name="enabled" valuePropName="checked">
              <Switch />
            </Form.Item>
            <Button htmlType="submit" type="primary">
              校验表单
            </Button>
          </Form>
          {notice && <p role="status">{notice}</p>}
        </SectionCard>
        <SectionCard title="软件封面与详情">
          <div className="rss-software-grid">
            <SoftwareCard
              title="开发工具"
              description="开发装配中的封面示例"
              cover={
                <SoftwareCover
                  title="专注创造"
                  subtitle="工具与工作流"
                  tone="workspace"
                />
              }
              onDetails={openDetails}
              action={<Button onClick={openDetails}>查看</Button>}
            />
            <SoftwareCard
              title="设计工具"
              description="开发装配中的封面示例"
              cover={
                <SoftwareCover
                  title="创意从这里开始"
                  subtitle="设计与灵感"
                  tone="design"
                />
              }
              onDetails={openDetails}
            />
          </div>
        </SectionCard>
        <SectionCard title="状态语义">
          <ActionStatus phase="received" />
          <ActionStatus phase="ended" />
          <ActionStatus phase="verified" />
          <ActionStatus phase="unknown" />
          <ActionStatus phase="cancelRequested" />
          <StatePanel
            kind="unknown"
            title="结果未知"
            description="只查询原请求，不重新提交。"
            onRead={() => setReads(reads + 1)}
          />
          <p role="status">查询次数：{reads}</p>
          <StatePanel
            kind="offline"
            title="离线"
            description="当前事实未重新核实。"
          />
          <StatePanel kind="readonly" title="只读" />
          <StatePanel kind="error" title="读取失败" />
          <StatePanel kind="empty" title="没有内容" />
          <StatePanel kind="loading" title="读取中" />
        </SectionCard>
        <SectionCard title="表格">
          <Table
            rowKey="id"
            columns={[
              { title: "名称", dataIndex: "name" },
              { title: "状态", dataIndex: "state" },
            ]}
            dataSource={[{ id: "one", name: "开发示例", state: "只读" }]}
            pagination={false}
          />
        </SectionCard>
        <SectionCard title="消息与输入">
          <MessageStream
            messages={[
              {
                id: "untrusted",
                kind: "assistant",
                stable: true,
                text: "<script>alert(1)</script> ![图片](https://example.invalid/pixel) [链接](javascript:alert(1))\n\n| 列 |\n| --- |\n| 内容 |",
              },
              ...sent.map((text, id) => ({
                id: String(id),
                kind: "user" as const,
                text,
              })),
            ]}
          />
          <MessageComposer
            value={draft}
            onChange={setDraft}
            onSend={() => {
              setSent([...sent, draft]);
              setDraft("");
            }}
          />
        </SectionCard>
        <DetailDrawer
          open={open}
          returnFocus={() => opener.current?.focus()}
          onClose={() => setOpen(false)}
          title="详情"
          description="当前仅验证产品组件"
          footer={<Button onClick={() => setOpen(false)}>完成</Button>}
        >
          <p>详情正文</p>
          <Input aria-label="详情字段" />
        </DetailDrawer>
      </main>
    </UiProvider>
  );
}
if (window.location.pathname === "/tests/assistant/")
  createRoot(document.getElementById("app")!).render(<FixtureApp />);
