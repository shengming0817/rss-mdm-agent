import { defineComponent, h, type PropType } from "vue";
import { ToolbarRoot, ToolbarButton } from "reka-ui";
import { UiButton } from "./primitives";
export const UiToolbar = defineComponent({
  props: { label: { type: String, required: true } },
  setup:
    (props, { slots }) =>
    () =>
      h(ToolbarRoot, { class: "ui-toolbar", "aria-label": props.label }, slots),
});
export const UiToolbarButton = defineComponent({
  props: { disabled: Boolean },
  setup:
    (props, { slots }) =>
    () =>
      h(ToolbarButton, { disabled: props.disabled }, slots),
});
export const UiProgress = defineComponent({
  props: {
    value: Number,
    max: { type: Number, default: 100 },
    label: { type: String, required: true },
  },
  setup: (props) => () =>
    h("progress", {
      class: "ui-progress",
      value:
        props.value === undefined
          ? undefined
          : Math.max(0, Math.min(props.max, props.value)),
      max: props.max > 0 ? props.max : 100,
      "aria-label": props.label,
    }),
});
export const UiSpinner = defineComponent({
  props: { label: { type: String, default: "处理中" } },
  setup: (props) => () =>
    h("span", { role: "status", class: "ui-busy" }, [
      h("span", { class: "ui-spinner", "aria-hidden": true }),
      h("span", { class: "sr-only" }, props.label),
    ]),
});
export const UiSteps = defineComponent({
  props: {
    items: {
      type: Array as PropType<readonly { id: string; label: string }[]>,
      required: true,
    },
    current: { type: String, required: true },
    label: { type: String, default: "步骤" },
  },
  setup: (props) => () =>
    h(
      "ol",
      { class: "ui-steps", "aria-label": props.label },
      props.items.map((item, index) =>
        h(
          "li",
          {
            key: item.id,
            "aria-current": item.id === props.current ? "step" : undefined,
          },
          [h("span", { "aria-hidden": true }, String(index + 1)), item.label],
        ),
      ),
    ),
});
export type UiTone = "neutral" | "success" | "warning" | "danger";
export const UiBadge = defineComponent({
  props: { tone: { type: String as PropType<UiTone>, default: "neutral" } },
  setup:
    (props, { slots }) =>
    () =>
      h(
        "span",
        { class: ["ui-badge", `ui-feedback-${props.tone}`] },
        slots.default?.(),
      ),
});
const marks = { neutral: "ⓘ", success: "✓", warning: "!", danger: "×" };
export const UiMessage = defineComponent({
  props: {
    tone: { type: String as PropType<UiTone>, default: "neutral" },
    dismissible: Boolean,
  },
  emits: ["dismiss"],
  setup:
    (props, { slots, emit }) =>
    () =>
      h(
        "div",
        {
          class: ["ui-message", `ui-feedback-${props.tone}`],
          role: props.tone === "danger" ? "alert" : "status",
        },
        [
          h("span", { "aria-hidden": true }, marks[props.tone]),
          h("div", slots.default?.()),
          props.dismissible
            ? h(
                UiButton,
                {
                  icon: true,
                  variant: "ghost",
                  "aria-label": "关闭消息",
                  onClick: () => emit("dismiss"),
                },
                () => "×",
              )
            : null,
        ],
      ),
});
export const UiEmptyState = defineComponent({
  props: {
    state: {
      type: String as PropType<"loading" | "empty" | "error">,
      required: true,
    },
    title: { type: String, required: true },
    retry: Boolean,
  },
  emits: ["retry"],
  setup:
    (props, { slots, emit }) =>
    () =>
      h(
        "div",
        {
          class: "ui-empty",
          role: props.state === "error" ? "alert" : "status",
          "aria-busy": props.state === "loading" || undefined,
        },
        [
          props.state === "loading"
            ? h("div", { class: "ui-skeleton", "aria-hidden": true })
            : null,
          h("p", props.title),
          slots.default?.(),
          props.retry && props.state !== "loading"
            ? h(UiButton, { onClick: () => emit("retry") }, () => "重试")
            : null,
        ],
      ),
});
export const UiSkeleton = defineComponent({
  setup: () => () => h("div", { class: "ui-skeleton", "aria-hidden": true }),
});
