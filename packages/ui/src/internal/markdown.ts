import MarkdownIt, { type Token } from "markdown-it";

// ref: markdown-it src/markdownit.ts@15.0.2 — parse tokens, never rendered HTML.
const parser = new MarkdownIt({
  html: false,
  linkify: false,
  typographer: false,
});
export interface MarkdownNode {
  kind: string;
  text: string;
  children: MarkdownNode[];
  url: string;
  level: number;
  start: number;
}
function nodes(tokens: readonly Token[]): MarkdownNode[] {
  const result: MarkdownNode[] = [];
  const stack = [result];
  for (const token of tokens) {
    if (token.nesting === -1) {
      if (stack.length > 1) stack.pop();
      continue;
    }
    if (token.type === "inline") {
      stack.at(-1)!.push(...nodes(token.children ?? []));
      continue;
    }
    const node: MarkdownNode = {
      kind: token.type.replace(/_open$/, ""),
      text: token.content,
      children: [],
      url: token.type === "link_open" ? `${token.attrGet("href") ?? ""}` : "",
      level: token.type === "heading_open" ? Number(token.tag.slice(1)) : 0,
      start: Number(token.attrGet("start") ?? 1),
    };
    stack.at(-1)!.push(node);
    if (token.nesting === 1) stack.push(node.children);
  }
  return result;
}
export function parseMarkdown(text: string): MarkdownNode[] {
  try {
    return nodes(parser.parse(text, {}));
  } catch {
    return [{ kind: "text", text, children: [], url: "", level: 0, start: 1 }];
  }
}
