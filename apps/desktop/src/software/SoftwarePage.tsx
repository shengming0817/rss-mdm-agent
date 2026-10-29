import { useEffect, useRef, useState } from "react";
import { Alert, Button, Input, Segmented, Select, Table, Tag } from "antd";
import {
  AppstoreOutlined,
  ArrowRightOutlined,
  BarsOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useSnapshot } from "valtio";
import type { Snapshot } from "valtio/vanilla";
import {
  PageHeader,
  SoftwareCard,
  SoftwareCover,
  StatePanel,
} from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import type {
  InstalledSoftware,
  SelfServiceDirectoryItem,
} from "../self-service/types";
import {
  categories,
  localAction,
  policyPresentation,
  scopeLabel,
} from "./presentation";
import { SoftwareDetail } from "./SoftwareDetail";
import { SoftwareActions } from "./SoftwareActions";
import "./software.css";

type Selection = { kind: "installed" | "policy"; id: string };
export function SoftwarePage({
  workspace,
  section,
  active,
}: {
  workspace: Workspace;
  section: string;
  active: boolean;
}) {
  const software = useSnapshot(workspace.software.state),
    nav = useSnapshot(workspace.navigation.state),
    directory = useSnapshot(workspace.directory.state),
    [query, setQuery] = useState(""),
    [category, setCategory] = useState("all"),
    [layout, setLayout] = useState("cards"),
    [selection, setSelection] = useState<Selection | null>(null),
    [message, setMessage] = useState(""),
    previousSection = useRef(section),
    catalogue = section === "discover" || section.startsWith("category:"),
    inventory = workspace.software.summary(software),
    installed = inventory.items,
    policies = directory.items.filter((item) => item.task.kind === "software"),
    detailInstalled =
      selection?.kind === "installed"
        ? installed.find((item) => item.reference.installation === selection.id)
        : undefined,
    detailPolicy =
      selection?.kind === "policy"
        ? policies.find((item) => item.policyId === selection.id)
        : undefined;
  useEffect(() => {
    if (!active) return;
    if (catalogue && !workspace.directory.state.readAt)
      void workspace.directory.load();
    if (!catalogue && !workspace.software.state.sources.length)
      void workspace.software.discover();
  }, [workspace, active, catalogue]);
  useEffect(() => {
    if (previousSection.current !== section) {
      previousSection.current = section;
      setSelection(null);
      setQuery("");
    }
  }, [section]);
  useEffect(() => {
    const target = nav.target;
    if (!active || target?.kind !== "resource" || target.page !== "software")
      return;
    if (target.id.startsWith("policy:")) {
      const item = policies.find(
        (value) =>
          `policy:${value.policyId}` === target.id &&
          JSON.stringify([
            value.policyId,
            value.policyVersion,
            value.policyRevision,
            value.subject,
          ]) === target.revision,
      );
      if (item) setSelection({ kind: "policy", id: item.policyId });
    } else {
      const item = installed.find(
        (value) =>
          value.reference.installation === target.id &&
          JSON.stringify(value.reference) === target.revision,
      );
      if (item)
        setSelection({ kind: "installed", id: item.reference.installation });
    }
  }, [active, nav.target]);
  useEffect(() => {
    const preparation = nav.preparation?.input;
    if (!active || preparation?.kind !== "software") return;
    if (preparation.software) {
      const item = installed.find(
        (value) =>
          JSON.stringify(value.reference) ===
          JSON.stringify(preparation.software),
      );
      if (item)
        setSelection({ kind: "installed", id: item.reference.installation });
      else
        setMessage(
          "入口选择的原安装项尚未在当前清单中取得，请重读后核对。未发起操作。",
        );
    }
    workspace.navigation.clearPreparation();
  }, [active, nav.preparation]);
  const read = () => {
    if (catalogue) void workspace.directory.load();
    else if (!workspace.software.state.sources.length)
      void workspace.software.discover();
    else
      for (const source of workspace.software.state.sources)
        void workspace.software.read(source);
  };
  const show = (kind: Selection["kind"], id: string) => {
      setSelection({ kind, id });
      if (kind === "installed") void workspace.software.history();
    },
    textMatches = (text: string) =>
      text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
    effectiveCategory = section.startsWith("category:")
      ? section.slice(9)
      : category,
    visiblePolicies = policies.filter((item) => {
      const view = policyPresentation(item);
      return (
        textMatches(`${view.name} ${view.description} ${view.publisher}`) &&
        (effectiveCategory === "all" || view.category === effectiveCategory)
      );
    }),
    visibleInstalled = installed.filter(
      (item) =>
        textMatches(`${item.name} ${item.publisher ?? ""} ${item.nativeId}`) &&
        (section !== "updates" ||
          (localAction(item, "update").available &&
            !!localAction(item, "update").version)),
    ),
    error = catalogue ? directory.error : inventory.error,
    loading = catalogue ? directory.loading : inventory.loading;
  const localActions = (item: Snapshot<InstalledSoftware>) => (
    <SoftwareActions
      active={active}
      workspace={workspace}
      installed={item}
      updates={section === "updates"}
    />
  );
  return (
    <>
      {selection ? (
        detailInstalled || detailPolicy ? (
          <SoftwareDetail
            key={`${selection.kind}:${selection.id}`}
            workspace={workspace}
            installed={detailInstalled}
            policy={detailPolicy}
            onBack={() => setSelection(null)}
            actions={
              <SoftwareActions
                active={active}
                workspace={workspace}
                installed={detailInstalled}
                policy={detailPolicy}
              />
            }
          />
        ) : (
          <div className="rss-page">
            <StatePanel
              kind="unknown"
              title="原详情不在当前读取范围"
              description="目录或安装清单已变化，请返回后重新核对。"
            />
            <Button onClick={() => setSelection(null)}>返回软件中心</Button>
          </div>
        )
      ) : (
        <div className="rss-page rss-software-page">
          <p className="rss-eyebrow">SOFTWARE LIBRARY</p>
          <PageHeader
            title="软件中心"
            description={
              catalogue
                ? "发现公司发布的工作应用，按用途浏览并核对安装范围。"
                : section === "updates"
                  ? "仅显示本机已安装软件的获准固定更新候选。"
                  : "本机已安装的软件，打开、查看和卸载集中在同一页。"
            }
            actions={
              <>
                <Button
                  icon={<ReloadOutlined />}
                  disabled={loading}
                  onClick={read}
                >
                  重读结果
                </Button>
                {!catalogue && (
                  <Button
                    type="primary"
                    disabled={
                      !software.sources.some(
                        (source) => source.descriptor.available,
                      ) || loading
                    }
                    onClick={() => {
                      for (const source of workspace.software.state.sources)
                        void workspace.software.refreshSource(source);
                    }}
                  >
                    采集最新清单
                  </Button>
                )}
              </>
            }
          />
          {section === "discover" && !query && (
            <div className="rss-store-editorial">
              <section className="rss-software-story">
                <span className="rss-eyebrow">高效工作 · 按用途探索</span>
                <h2>
                  好工具，
                  <br />
                  让工作轻一点。
                </h2>
                <p>
                  沟通、记录、专注。
                  <br />
                  为你的每一天，找到恰到好处的应用。
                </p>
                <Button
                  onClick={() =>
                    workspace.navigation.select("software", "category:效率办公")
                  }
                >
                  探索效率应用 <ArrowRightOutlined />
                </Button>
                <div className="rss-story-icons" aria-hidden="true">
                  <i>W</i>
                  <i>N</i>
                  <i>↻</i>
                </div>
              </section>
              <section className="rss-creative-story">
                <span className="rss-eyebrow">CREATIVE SPACE</span>
                <h2>
                  每个想法，
                  <br />
                  都值得被看见。
                </h2>
                <p>
                  从第一笔灵感，到下一件作品。
                  <br />
                  发现你的创作搭档。
                </p>
                <Button
                  type="link"
                  onClick={() =>
                    workspace.navigation.select("software", "category:创意设计")
                  }
                >
                  探索创意应用 →
                </Button>
                <div className="rss-creative-flower" aria-hidden="true">
                  <i />
                  <i />
                  <i />
                  <i />
                </div>
              </section>
            </div>
          )}
          <div className="rss-software-section-heading">
            <div>
              <h2>
                {section.startsWith("category:")
                  ? section.slice(9)
                  : section === "discover"
                    ? "开始你的高效工作日"
                    : section === "updates"
                      ? "可用更新"
                      : "已安装应用"}
              </h2>
              <p>
                {catalogue
                  ? "公司发布目录 · 当前主体可见 · 先查看版本与作用范围"
                  : "本机安装事实 · 与组织发布目录分别读取"}
              </p>
            </div>
            {!catalogue && (
              <Segmented
                aria-label="软件显示方式"
                value={layout}
                onChange={setLayout}
                options={[
                  { value: "cards", icon: <AppstoreOutlined />, label: "卡片" },
                  { value: "table", icon: <BarsOutlined />, label: "列表" },
                ]}
              />
            )}
          </div>
          <div className="rss-software-filterbar">
            <Input.Search
              aria-label="搜索软件"
              placeholder={
                catalogue ? "搜索公司发布的应用" : "搜索本机已安装软件"
              }
              allowClear
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            {catalogue && (
              <Select
                aria-label="软件分类"
                value={effectiveCategory}
                options={[
                  { label: "全部分类", value: "all" },
                  ...categories.map((name) => ({ label: name, value: name })),
                  { label: "未分类", value: "未分类" },
                ]}
                onChange={(value) => {
                  if (section.startsWith("category:"))
                    workspace.navigation.select("software", "discover");
                  setCategory(value);
                }}
              />
            )}
            <span>
              {catalogue ? visiblePolicies.length : visibleInstalled.length}{" "}
              项已读取结果
            </span>
          </div>
          <p className="rss-software-source">
            {catalogue
              ? `授权 Policy 目录 · ${directory.asOf ? new Date(directory.asOf * 1000).toLocaleString() : "尚未取得来源时间"} · ${directory.complete ? "当前目录已读完" : "覆盖未完整"}`
              : `${inventory.sources} 个本机来源 · ${inventory.collectedAt == null ? "观察时间未取得" : new Date(inventory.collectedAt * 1000).toLocaleString()} · ${inventory.complete ? "完整观察" : "部分或尚未读取"}`}
          </p>
          {(message || error) && (
            <Alert
              type="warning"
              title={message || error}
              description={
                <Button size="small" onClick={read}>
                  重新读取
                </Button>
              }
            />
          )}
          {loading &&
          !(catalogue ? visiblePolicies.length : visibleInstalled.length) ? (
            <StatePanel
              kind="loading"
              title={catalogue ? "正在读取发布目录" : "正在读取安装清单"}
            />
          ) : catalogue ? (
            visiblePolicies.length ? (
              <div className="rss-store-grid">
                {visiblePolicies.map((item) => {
                  const view = policyPresentation(item);
                  return (
                    <SoftwareCard
                      key={item.policyId}
                      title={view.name}
                      description={`${view.publisher} · ${view.category}`}
                      cover={
                        <SoftwareCover
                          title={view.name}
                          subtitle={view.category}
                          tone={view.tone}
                        />
                      }
                      onDetails={() => show("policy", item.policyId)}
                      trailing={
                        <SoftwareActions
                          active={active}
                          workspace={workspace}
                          policy={item}
                        />
                      }
                    />
                  );
                })}
              </div>
            ) : (
              <StatePanel
                kind={error ? "error" : "empty"}
                title="当前范围没有可见应用"
                description={
                  query || effectiveCategory !== "all"
                    ? "可以清空搜索或切换分类；仅使用发布方明确提供的分类。"
                    : "尚未读到公司发布的软件项目。可重读目录，或查看本机已安装的软件。"
                }
                onRead={read}
              />
            )
          ) : visibleInstalled.length ? (
            layout === "cards" ? (
              <div className="rss-installed-grid">
                {visibleInstalled.map((item) => (
                  <article
                    className="rss-installed-card"
                    key={item.reference.installation}
                  >
                    <div className="rss-installed-card-top">
                      <span className="rss-app-icon">
                        {item.name.slice(0, 1)}
                      </span>
                      <Tag>
                        {localAction(item, "update").available
                          ? "有可用更新"
                          : "已读取安装"}
                      </Tag>
                    </div>
                    <Button
                      type="text"
                      className="rss-app-name"
                      onClick={() =>
                        show("installed", item.reference.installation)
                      }
                    >
                      {item.name}
                    </Button>
                    <p>{item.publisher ?? "发布方未提供"}</p>
                    <p>
                      {item.version ?? "版本未提供"} · {scopeLabel(item.scope)}
                    </p>
                    <small>{item.origin}</small>
                    <div className="rss-installed-card-actions">
                      <Button
                        size="small"
                        onClick={() =>
                          show("installed", item.reference.installation)
                        }
                      >
                        详情
                      </Button>
                      {localActions(item)}
                    </div>
                    {!localAction(item, "uninstall").available && (
                      <p className="rss-software-protection">
                        {localAction(item, "uninstall").reason}
                      </p>
                    )}
                  </article>
                ))}
              </div>
            ) : (
              <Table
                rowKey={(item) => item.reference.installation}
                dataSource={visibleInstalled}
                pagination={false}
                scroll={{ x: 700 }}
                columns={[
                  {
                    title: "应用名称",
                    render: (_, item) => (
                      <Button
                        type="link"
                        onClick={() =>
                          show("installed", item.reference.installation)
                        }
                      >
                        {item.name}
                      </Button>
                    ),
                  },
                  {
                    title: "安装版本",
                    render: (_, item) => (
                      <>
                        {item.version ?? "未提供"}
                        {localAction(item, "update").version && (
                          <small className="rss-table-sub">
                            更新至 {localAction(item, "update").version}
                          </small>
                        )}
                      </>
                    ),
                  },
                  {
                    title: "来源 / 范围",
                    render: (_, item) => (
                      <>
                        {item.origin}
                        <small className="rss-table-sub">
                          {scopeLabel(item.scope)}
                        </small>
                      </>
                    ),
                  },
                  {
                    title: "操作",
                    render: (_, item) => (
                      <div className="rss-inline-actions">
                        {localActions(item)}
                      </div>
                    ),
                  },
                ]}
              />
            )
          ) : (
            <StatePanel
              kind={error ? "error" : "empty"}
              title={
                section === "updates"
                  ? "当前读取范围没有获准更新候选"
                  : "当前范围没有已读取的软件"
              }
              description="完整空清单与尚未取得记录分别按来源状态呈现。"
              onRead={read}
            />
          )}
          <div className="rss-software-pagination">
            {catalogue ? (
              directory.next && (
                <Button
                  disabled={directory.loading}
                  onClick={() => void workspace.directory.load(true)}
                >
                  读取更多发布项目
                </Button>
              )
            ) : (
              <>
                {software.directoryNext && (
                  <Button
                    onClick={() => void workspace.software.discover(true)}
                  >
                    读取更多来源
                  </Button>
                )}
                {software.sources.map(
                  (source) =>
                    source.after && (
                      <Button
                        key={JSON.stringify(source.descriptor.reference)}
                        disabled={source.busy}
                        onClick={() => {
                          const original =
                            workspace.software.state.sources.find(
                              (value) =>
                                JSON.stringify(value.descriptor.reference) ===
                                JSON.stringify(source.descriptor.reference),
                            );
                          if (original)
                            void workspace.software.read(original, true);
                        }}
                      >
                        读取更多安装记录
                      </Button>
                    ),
                )}
              </>
            )}
          </div>
          {!catalogue &&
            [...software.queries.values()].map((value) => (
              <Alert
                key={value.request}
                type={value.phase === "unknown" ? "warning" : "info"}
                title={`原采集请求 · ${value.phase}`}
                description={
                  <Button
                    onClick={() =>
                      void workspace.software.queryRefresh(value.request)
                    }
                  >
                    查询原采集
                  </Button>
                }
              />
            ))}
          {section === "discover" && (
            <>
              <div className="rss-software-section-heading">
                <h2>按你的需要，发现应用</h2>
              </div>
              <div className="rss-category-chips">
                {categories.map((name) => (
                  <Button
                    key={name}
                    onClick={() =>
                      workspace.navigation.select(
                        "software",
                        `category:${name}`,
                      )
                    }
                  >
                    {name} <ArrowRightOutlined />
                  </Button>
                ))}
              </div>
              <button
                type="button"
                className="rss-library-banner"
                onClick={() =>
                  workspace.navigation.select("software", "installed")
                }
              >
                <span>
                  <strong>你的工作搭档，已经准备好了。</strong>
                  <small>查看、打开或卸载本机已安装的软件。</small>
                </span>
                <span>进入已安装 →</span>
              </button>
            </>
          )}
        </div>
      )}
    </>
  );
}
