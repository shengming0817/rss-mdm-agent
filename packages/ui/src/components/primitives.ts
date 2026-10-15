import { defineComponent, h, type PropType } from "vue";

export const UiText = defineComponent({
  props: {
    as: {
      type: String as PropType<"span" | "p" | "h1" | "h2" | "h3">,
      default: "span",
    },
    tone: { type: String, default: "default" },
    truncate: Boolean,
  },
  setup:
    (props, { slots }) =>
    () =>
      h(
        props.as,
        {
          class: [
            "ui-text",
            `ui-tone-${props.tone}`,
            { "ui-truncate": props.truncate },
          ],
        },
        slots.default?.(),
      ),
});
export const UiIcon = defineComponent({
  props: { label: String, size: { type: Number, default: 18 } },
  setup:
    (props, { slots }) =>
    () =>
      h(
        "span",
        {
          class: "ui-icon",
          role: props.label ? "img" : undefined,
          "aria-label": props.label,
          "aria-hidden": props.label ? undefined : true,
          style: { width: `${props.size}px`, height: `${props.size}px` },
        },
        slots.default?.(),
      ),
});
export const UiStack = defineComponent({
  props: {
    direction: {
      type: String as PropType<"row" | "column">,
      default: "column",
    },
    gap: { type: Number, default: 12 },
    align: {
      type: String as PropType<"start" | "center" | "end" | "stretch">,
      default: "stretch",
    },
    wrap: Boolean,
  },
  setup:
    (props, { slots }) =>
    () =>
      h(
        "div",
        {
          class: "ui-stack",
          style: {
            flexDirection: props.direction,
            gap: `${props.gap}px`,
            alignItems: props.align,
            flexWrap: props.wrap ? "wrap" : "nowrap",
          },
        },
        slots.default?.(),
      ),
});
export const UiGrid = defineComponent({
  props: {
    minWidth: { type: Number, default: 180 },
    gap: { type: Number, default: 12 },
  },
  setup:
    (props, { slots }) =>
    () =>
      h(
        "div",
        {
          class: "ui-grid",
          style: {
            gridTemplateColumns: `repeat(auto-fit,minmax(min(100%,${props.minWidth}px),1fr))`,
            gap: `${props.gap}px`,
          },
        },
        slots.default?.(),
      ),
});
export const UiSurface = defineComponent({
  setup:
    (_, { slots }) =>
    () =>
      h("div", { class: "ui-surface" }, slots.default?.()),
});
export const UiDivider = defineComponent({
  props: { vertical: Boolean },
  setup: (props) => () =>
    h("div", {
      class: "ui-divider",
      role: "separator",
      "aria-orientation": props.vertical ? "vertical" : "horizontal",
    }),
});
export const UiButton = defineComponent({
  props: {
    variant: {
      type: String as PropType<"default" | "primary" | "danger" | "ghost">,
      default: "default",
    },
    disabled: Boolean,
    busy: Boolean,
    icon: Boolean,
    type: {
      type: String as PropType<"button" | "submit" | "reset">,
      default: "button",
    },
  },
  setup:
    (props, { slots }) =>
    () =>
      h(
        "button",
        {
          class: [
            "ui-button",
            `ui-button-${props.variant}`,
            { "ui-icon-button": props.icon },
          ],
          type: props.type,
          disabled: props.disabled || props.busy,
          "aria-busy": props.busy || undefined,
        },
        [
          props.busy
            ? h("span", { class: "ui-spinner", "aria-hidden": true })
            : null,
          slots.default?.(),
        ],
      ),
});
export const UiLink = defineComponent({
  props: { disabled: Boolean },
  emits: ["activate"],
  setup:
    (props, { slots, emit }) =>
    () =>
      h(
        "button",
        {
          class: "ui-link",
          type: "button",
          role: "link",
          disabled: props.disabled,
          onClick: () => emit("activate"),
        },
        slots.default?.(),
      ),
});
