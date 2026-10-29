import { Button } from "antd";
import { useState, type ReactNode } from "react";
import { parseMarkdown, type MarkdownNode } from "../internal/markdown";

export function MarkdownContent({
  text,
  stable,
  onCopy,
}: {
  text: string;
  stable: boolean;
  onCopy?(text: string): void;
}) {
  const [expanded, setExpanded] = useState(true);
  const copy = (value: string, label: string) =>
    onCopy && (
      <Button type="text" size="small" onClick={() => onCopy(value)}>
        {label}
      </Button>
    );
  function render(node: MarkdownNode, key: number): ReactNode {
    const children = node.children.map(render);
    switch (node.kind) {
      case "paragraph":
        return <p key={key}>{children}</p>;
      case "heading":
        return <h3 key={key}>{children}</h3>;
      case "bullet_list":
        return <ul key={key}>{children}</ul>;
      case "ordered_list":
        return (
          <ol key={key} start={node.start}>
            {children}
          </ol>
        );
      case "list_item":
        return <li key={key}>{children}</li>;
      case "blockquote":
        return <blockquote key={key}>{children}</blockquote>;
      case "strong":
        return <strong key={key}>{children}</strong>;
      case "em":
        return <em key={key}>{children}</em>;
      case "s":
        return <s key={key}>{children}</s>;
      case "code_inline":
        return <code key={key}>{node.text}</code>;
      case "fence":
      case "code_block":
        return (
          <section key={key}>
            {copy(node.text, "复制代码")}
            <pre>
              <code>{node.text}</code>
            </pre>
          </section>
        );
      case "table":
        return (
          <div
            key={key}
            className="rss-table-scroll"
            tabIndex={0}
            aria-label="消息表格"
          >
            <table>{children}</table>
          </div>
        );
      case "thead":
        return <thead key={key}>{children}</thead>;
      case "tbody":
        return <tbody key={key}>{children}</tbody>;
      case "tr":
        return <tr key={key}>{children}</tr>;
      case "th":
        return <th key={key}>{children}</th>;
      case "td":
        return <td key={key}>{children}</td>;
      case "link":
        return (
          <span key={key}>
            {children}
            <span className="rss-muted"> ({node.url})</span>
            {copy(node.url, "复制链接")}
          </span>
        );
      case "image":
        return <span key={key}>[图片：{node.text}]</span>;
      case "hr":
        return <hr key={key} />;
      case "hardbreak":
        return <br key={key} />;
      case "softbreak":
        return "\n";
      default:
        return node.text;
    }
  }
  const long = text.length > 2000 || text.split("\n").length > 24;
  return (
    <div className="rss-markdown" data-stable={stable}>
      {long && (
        <Button
          type="text"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? "收起长内容" : "展开长内容"}
        </Button>
      )}
      {expanded ? (
        parseMarkdown(text).map(render)
      ) : (
        <p>{text.slice(0, 160)}…</p>
      )}
    </div>
  );
}
