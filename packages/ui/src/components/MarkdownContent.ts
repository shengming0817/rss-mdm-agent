import { computed, defineComponent, h, ref, type VNodeChild } from "vue";
import { parseMarkdown, type MarkdownNode } from "../internal/markdown";

export default defineComponent({
  name: "MarkdownContent",
  props: {
    text: { type: String, required: true },
    stable: { type: Boolean, required: true },
  },
  emits: { copy: (_text: string) => true },
  setup(props, { emit }) {
    const content = computed(() => parseMarkdown(props.text));
    const expanded = ref(true);
    const long = computed(
      () => props.text.length > 2000 || props.text.split("\n").length > 24,
    );
    const render = (node: MarkdownNode): VNodeChild => {
      const children = node.children.map(render);
      switch (node.kind) {
        case "paragraph":
          return h("p", children);
        case "heading":
          switch (node.level) {
            case 1:
              return h("h2", children);
            case 2:
              return h("h3", children);
            case 3:
              return h("h4", children);
            case 4:
              return h("h5", children);
            default:
              return h("h6", children);
          }
        case "bullet_list":
          return h("ul", children);
        case "ordered_list":
          return h("ol", { start: node.start }, children);
        case "list_item":
          return h("li", children);
        case "blockquote":
          return h("blockquote", children);
        case "strong":
          return h("strong", children);
        case "em":
          return h("em", children);
        case "s":
          return h("s", children);
        case "code_inline":
          return h("code", node.text);
        case "fence":
        case "code_block":
          return h("section", { class: "code-block" }, [
            h(
              "button",
              {
                type: "button",
                "aria-label": "复制代码",
                onClick: () => emit("copy", node.text),
              },
              "复制代码",
            ),
            h("pre", [h("code", node.text)]),
          ]);
        case "table":
          return h(
            "div",
            { class: "table-scroll", tabindex: 0, "aria-label": "消息表格" },
            [h("table", children)],
          );
        case "thead":
          return h("thead", children);
        case "tbody":
          return h("tbody", children);
        case "tr":
          return h("tr", children);
        case "th":
          return h("th", children);
        case "td":
          return h("td", children);
        case "link":
          return h("span", { class: "message-link" }, [
            ...children,
            h("span", { class: "muted" }, ` (${node.url})`),
            h(
              "button",
              {
                type: "button",
                "aria-label": "复制链接",
                onClick: () => emit("copy", node.url),
              },
              "复制链接",
            ),
          ]);
        case "image":
          return h("span", { class: "muted" }, `[图片：${node.text}]`);
        case "hr":
          return h("hr");
        case "hardbreak":
          return h("br");
        case "softbreak":
          return "\n";
        default:
          return node.text;
      }
    };
    return () =>
      h("div", { class: "markdown-message", "data-stable": props.stable }, [
        long.value &&
          h(
            "button",
            {
              type: "button",
              "aria-expanded": expanded.value,
              onClick: () => {
                expanded.value = !expanded.value;
              },
            },
            expanded.value ? "收起长内容" : "展开长内容",
          ),
        expanded.value
          ? content.value.map(render)
          : h("p", props.text.slice(0, 160) + "…"),
      ]);
  },
});
