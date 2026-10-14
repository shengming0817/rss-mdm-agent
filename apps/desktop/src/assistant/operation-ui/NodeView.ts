import {
  createApp,
  defineComponent,
  h,
  inject,
  nextTick,
  onBeforeUnmount,
  ref,
  shallowRef,
  useId,
  watch,
  type InjectionKey,
  type PropType,
  type VNodeChild,
} from "vue";
import {
  effect,
  getValue,
  ResolvedBinding,
  WritableBinding,
  type ComponentNode,
} from "@a2ui/web_core/v0_9";
import type {
  OperationController,
  NodeMount,
} from "@rss-mdm-agent/ai-ui-bridge";
import type { OperationValue } from "@rss-mdm-agent/ai-contract/operations";
import "./style.css";
export const operationContext: InjectionKey<OperationController> =
  Symbol("operation");
const valueOf = (v: unknown): unknown =>
  v instanceof ResolvedBinding ? v.value : v;
const display = (v: unknown) =>
  v === undefined || v === null
    ? "—"
    : Array.isArray(v)
      ? v.join("、")
      : String(v);
const phaseLabels: Record<string, string> = {
  ready: "准备就绪",
  submitting: "提交中",
  waiting: "等待交互",
  running: "执行中",
  failed: "失败",
  unknown: "结果未知",
  succeeded: "核实成功",
  pending_effect: "待生效",
  cancelled: "已取消",
  denied: "已拒绝",
};

export const NodeView = defineComponent({
  name: "OperationNode",
  props: { node: { type: Object as PropType<ComponentNode>, required: true } },
  setup(props) {
    const c = inject(operationContext, undefined);
    const p = shallowRef(getValue(props.node.props));
    let stop = () => {};
    watch(
      () => props.node,
      (node) => {
        stop();
        stop = effect(() => {
          p.value = getValue(node.props);
        });
      },
      { immediate: true },
    );
    onBeforeUnmount(() => stop());
    const domId = `op-${useId()}`,
      selectedTab = ref(0);
    watch(
      () => c?.state.step,
      () => {
        if (!c || props.node.type !== "Tabs") return;
        const step = c.presentation.steps?.at(c.state.step);
        const field = c.presentation.fields.find((f) =>
          step?.fields.includes(f.id),
        );
        const section = field?.section ?? c.presentation.sections.at(0)?.id;
        const nodes = (p.value.children ?? []) as ComponentNode[];
        const index = nodes.findIndex(
          (node) => node.componentId === `section-${section}`,
        );
        if (index >= 0) selectedTab.value = index;
      },
    );
    const child = (node: ComponentNode): VNodeChild =>
      h(NodeView, { node, key: node.instanceId });
    const children = () =>
      ((p.value.children ?? []) as ComponentNode[]).map(child);
    const button = (label: string, action: () => void, disabled = false) =>
      h("button", { type: "button", disabled, onClick: action }, label);
    async function focusError(ids?: string[]) {
      if (!c) return;
      const id = Object.keys(c.state.errors).find(
        (id) => !ids || ids.includes(id),
      );
      if (!id) return;
      if (c.presentation.template === "GuidedFlow" && !c.state.fallbackReason) {
        const step = c.presentation.steps?.findIndex((step) =>
          step.fields.includes(id),
        );
        if (step !== undefined && step >= 0) c.state.step = step;
      }
      // ref: Vue nextTick flushes the step/container update before locating controls.
      await nextTick();
      const card = document.getElementById(domId)?.closest(".operation-card");
      const field = Array.from(
        card?.querySelectorAll<HTMLElement>("[data-operation-field]") ?? [],
      ).find((field) => field.getAttribute("data-operation-field") === id);
      if (!field) return;
      for (
        let parent = field.parentElement;
        parent && parent !== card;
        parent = parent.parentElement
      ) {
        if (parent.tagName === "DETAILS") parent.setAttribute("open", "");
        if (parent.getAttribute("role") === "tabpanel") {
          const tab = Array.from(
            card?.querySelectorAll<HTMLElement>('[role="tab"]') ?? [],
          ).find((tab) => tab.getAttribute("aria-controls") === parent?.id);
          tab?.click();
        }
      }
      await nextTick();
      field
        .querySelector<HTMLElement>(
          'input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[role="radiogroup"]',
        )
        ?.focus();
    }
    function action(
      kind: "submit" | "cancel" | "refresh" | "answer",
      answer?: string,
    ) {
      if (c) void c.act(kind, answer).then(() => focusError());
    }
    return (): VNodeChild => {
      const n = props.node,
        propsValue = p.value;
      if (n.state !== "resolved") throw new Error("操作组件无法解析");
      const type = n.type,
        s = c?.state;
      if (type === "Text")
        return h("span", {}, display(valueOf(propsValue.text)));
      if (type === "Column" || type === "FieldGroup")
        return h(
          type === "FieldGroup" ? "fieldset" : "div",
          { class: "operation-fields" },
          children(),
        );
      if (type === "Button")
        return h(
          "button",
          {
            type: "button",
            onClick: () => (propsValue.action as () => void)?.(),
          },
          propsValue.child
            ? [child(propsValue.child as ComponentNode)]
            : display(propsValue.label),
        );
      if (type === "OperationSurface")
        return h(
          "section",
          { class: "operation-surface", "aria-label": s?.snapshot.title },
          children(),
        );
      if (type === "Section")
        return h("section", { class: "operation-section" }, [
          h("h3", display(propsValue.label)),
          ...children(),
        ]);
      if (type === "Disclosure")
        return h("details", { open: true }, [
          h("summary", display(propsValue.label)),
          ...children(),
        ]);
      if (type === "Tabs") {
        const nodes = (propsValue.children ?? []) as ComponentNode[];
        const choose = (index: number) => {
          selectedTab.value = index;
          document.getElementById(`${domId}-tab-${index}`)?.focus();
        };
        return h("div", [
          h(
            "div",
            {
              role: "tablist",
              "aria-label": "设置分组",
              onKeydown: (e: KeyboardEvent) => {
                if (
                  ["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)
                ) {
                  e.preventDefault();
                  choose(
                    e.key === "Home"
                      ? 0
                      : e.key === "End"
                        ? nodes.length - 1
                        : (selectedTab.value +
                            (e.key === "ArrowRight" ? 1 : nodes.length - 1)) %
                          nodes.length,
                  );
                }
              },
            },
            nodes.map((node, i) =>
              h(
                "button",
                {
                  id: `${domId}-tab-${i}`,
                  type: "button",
                  role: "tab",
                  "aria-selected": i === selectedTab.value,
                  "aria-controls": `${domId}-panel-${i}`,
                  tabindex: i === selectedTab.value ? 0 : -1,
                  onClick: () => choose(i),
                },
                display(getValue(node.props).label),
              ),
            ),
          ),
          ...nodes.map((node, i) =>
            h(
              "div",
              {
                id: `${domId}-panel-${i}`,
                role: "tabpanel",
                "aria-labelledby": `${domId}-tab-${i}`,
                hidden: i !== selectedTab.value,
              },
              [child(node)],
            ),
          ),
        ]);
      }
      if (
        [
          "TextField",
          "NumberField",
          "SelectField",
          "Switch",
          "Checkbox",
          "RadioGroup",
          "PathPicker",
          "DateTimeField",
          "SecretField",
        ].includes(type)
      ) {
        const fieldId = n.componentId.startsWith("field-")
          ? n.componentId.slice(6)
          : "";
        const field = c?.presentation.fields.find((f) => f.id === fieldId);
        if (
          c?.presentation.template === "GuidedFlow" &&
          !c.state.fallbackReason &&
          field &&
          !c.presentation.steps?.[s!.step]?.fields.includes(fieldId)
        )
          return null;
        const binding = propsValue.text ?? propsValue.value;
        const current = field ? s!.draft.values[fieldId] : valueOf(binding);
        const secret = type === "SecretField";
        const variant =
          !field && type === "TextField" ? propsValue.variant : undefined;
        const disabled = c
          ? !c.editable(fieldId) || (secret && !c.secretAvailable)
          : !(binding instanceof WritableBinding);
        const error = s?.errors[fieldId];
        const write = (value: OperationValue) => {
          if (disabled) return;
          if (binding instanceof WritableBinding) binding.set(value);
          if (c) c.edit(fieldId, value);
        };
        const attrs = {
          id: domId,
          disabled,
          "aria-invalid": !!error,
          "aria-describedby": `${domId}-hint`,
          required: !!field?.required,
        };
        const label = field?.label ?? display(valueOf(propsValue.label));
        let input: VNodeChild;
        if (type === "RadioGroup")
          input = h(
            "div",
            { ...attrs, role: "radiogroup", "aria-label": label, tabindex: -1 },
            field?.options?.map((o) =>
              h("label", [
                h("input", {
                  type: "radio",
                  name: domId,
                  value: o.id,
                  checked: current === o.id,
                  disabled: disabled || !!o.disabledReason,
                  onChange: () => write(o.id),
                  "aria-describedby": `${domId}-hint`,
                }),
                o.label,
                o.disabledReason ? `（${o.disabledReason}）` : "",
              ]),
            ),
          );
        else if (type === "SelectField")
          input = h(
            "select",
            {
              ...attrs,
              value: current,
              onChange: (e: Event) =>
                write((e.target as HTMLSelectElement).value),
            },
            [
              h("option", { value: "" }, "请选择"),
              ...(field?.options ?? []).map((o) =>
                h(
                  "option",
                  { value: o.id, disabled: !!o.disabledReason },
                  o.label,
                ),
              ),
            ],
          );
        else if (["Switch", "Checkbox"].includes(type))
          input = h("input", {
            ...attrs,
            type: "checkbox",
            role: type === "Switch" ? "switch" : undefined,
            checked: current === true,
            onChange: (e: Event) =>
              write((e.target as HTMLInputElement).checked),
          });
        else
          input = h(variant === "longText" ? "textarea" : "input", {
            ...attrs,
            type:
              secret || variant === "obscured"
                ? "password"
                : type === "NumberField" || variant === "number"
                  ? "number"
                  : type === "DateTimeField"
                    ? "datetime-local"
                    : "text",
            value: secret ? undefined : (current ?? ""),
            autocomplete: secret ? "off" : undefined,
            min: field?.min,
            max: field?.max,
            maxlength: field?.maxLength ?? 8192,
            onInput: (e: Event) => {
              if (!secret) {
                const el = e.target as HTMLInputElement;
                write(
                  type === "NumberField"
                    ? el.value === ""
                      ? null
                      : el.valueAsNumber
                    : el.value,
                );
              }
            },
            onChange: secret
              ? (e: Event) => {
                  const el = e.target as HTMLInputElement;
                  const value = el.value;
                  el.value = "";
                  void c?.secret(fieldId, value);
                }
              : undefined,
          });
        return h(
          "div",
          { class: "operation-field", "data-operation-field": field?.id },
          [
            h("label", { for: domId }, label),
            input,
            type === "PathPicker"
              ? button(
                  "选择路径",
                  () => {
                    void c?.choosePath(fieldId);
                  },
                  disabled || !c?.pathAvailable,
                )
              : null,
            h(
              "p",
              {
                id: `${domId}-hint`,
                role: error ? "alert" : undefined,
                class: error ? "operation-error" : "operation-hint",
              },
              error ||
                field?.reason ||
                (field?.readonly
                  ? "只读"
                  : secret
                    ? s?.secrets[fieldId]
                      ? "已安全接收，内容不回显"
                      : c?.secretAvailable
                        ? "输入离开此卡片即清除"
                        : "秘密输入服务不可用"
                    : type === "PathPicker"
                      ? "选择路径不授予访问权限"
                      : type === "Switch"
                        ? field?.submitMode === "immediate"
                          ? "更改后立即提交"
                          : "保存时提交"
                        : ""),
            ),
          ],
        );
      }
      if (!c || !s) throw new Error("操作组件缺少宿主");
      const snapshot = s.snapshot;
      if (type === "Notice")
        return h(
          "div",
          { role: "status", "aria-live": "polite" },
          [
            s.fallbackReason,
            s.notice,
            ...Object.values(s.errors),
            snapshot.reason,
            s.stale ? "当前值已变化，请重新核对修改。" : "",
          ]
            .filter((value) => !!value)
            .map((message) => h("p", message)),
        );
      if (type === "ReadonlyRow")
        return h("dl", { class: "operation-summary" }, [
          h("dt", "操作"),
          h("dd", snapshot.title),
          h("dt", "来源"),
          h("dd", snapshot.source),
          h("dt", "目标"),
          h("dd", snapshot.targetLabel),
          h("dt", "影响"),
          h("dd", snapshot.impact),
        ]);
      if (type === "ApprovalStatus")
        return h("dl", { class: "operation-summary" }, [
          h("dt", "用户确认"),
          h("dd", snapshot.approval.user),
          h("dt", "企业批准"),
          h("dd", snapshot.approval.enterprise),
          h("dt", "系统提权"),
          h("dd", snapshot.approval.elevation),
        ]);
      if (type === "ObjectList" || type === "SelectableTable")
        return h("table", { class: "operation-table" }, [
          h("caption", "操作对象"),
          h("thead", [
            h("tr", [
              h("th", "选择"),
              h("th", "对象"),
              h("th", "详情"),
              h("th", "状态"),
            ]),
          ]),
          h(
            "tbody",
            snapshot.objects.map((o) =>
              h("tr", { key: o.id }, [
                h("td", [
                  h("input", {
                    type:
                      c.presentation.selection === "single"
                        ? "radio"
                        : "checkbox",
                    name: domId,
                    "aria-label": `选择 ${o.label}`,
                    checked: s.draft.selection.includes(o.id),
                    disabled: !c.can("submit") || !!o.disabledReason,
                    onChange: () => c.select(o.id),
                  }),
                ]),
                h("th", { scope: "row" }, o.label),
                h("td", o.detail),
                h("td", o.disabledReason || o.status || "可用"),
              ]),
            ),
          ),
        ]);
      if (type === "ChangePreview")
        return h("table", { class: "operation-table" }, [
          h("caption", "修改预览"),
          h("thead", [
            h("tr", [h("th", "参数"), h("th", "当前"), h("th", "提交值")]),
          ]),
          h(
            "tbody",
            Object.entries(s.draft.values).map(([id, value]) =>
              h("tr", { key: id }, [
                h(
                  "th",
                  { scope: "row" },
                  c.presentation.fields.find((f) => f.id === id)?.label ?? id,
                ),
                h("td", display(snapshot.current[id])),
                h("td", display(value)),
              ]),
            ),
          ),
        ]);
      if (type === "ConfirmationPanel")
        return h(
          "aside",
          { "aria-label": "操作确认" },
          "请核对目标和最终参数。确认仅表达操作意图，不替代企业批准或系统提权。",
        );
      if (type === "Progress")
        return h("div", { role: "status" }, [
          h(
            "span",
            s.busy
              ? "提交中"
              : s.unknown
                ? "结果未知"
                : phaseLabels[snapshot.phase],
          ),
          snapshot.progress
            ? h("progress", {
                value: snapshot.progress.completed,
                max: snapshot.progress.total,
                "aria-label": "任务进度",
              })
            : null,
        ]);
      if (type === "ResultSummary")
        return snapshot.result
          ? h("p", { role: "status" }, snapshot.result)
          : null;
      if (type === "LogView")
        return h("details", [
          h("summary", "操作日志"),
          h(
            "pre",
            { role: "log", "aria-label": "操作日志" },
            snapshot.logs.join("\n"),
          ),
        ]);
      if (type === "PendingInteraction")
        return snapshot.interaction
          ? h("fieldset", [
              h("legend", snapshot.interaction.question),
              ...snapshot.interaction.options.map((o) =>
                button(o.label, () => action("answer", o.id), !c.can("answer")),
              ),
            ])
          : null;
      if (type === "StepFlow")
        return h("nav", { "aria-label": "操作步骤", id: domId }, [
          h(
            "ol",
            c.presentation.steps?.map((step, i) =>
              h(
                "li",
                { "aria-current": i === s.step ? "step" : undefined },
                step.label,
              ),
            ),
          ),
          button("上一步", c.previous, s.step === 0),
          button(
            "下一步",
            () => {
              const ids = c.presentation.steps?.at(s.step)?.fields;
              c.next();
              void focusError(ids);
            },
            s.step >= (c.presentation.steps?.length ?? 1) - 1,
          ),
        ]);
      if (type === "ActionBar")
        return h("div", { class: "operation-actions", id: domId }, [
          s.stale
            ? button("重新核对修改", c.rebase, s.busy || s.unknown)
            : null,
          button("刷新状态", () => action("refresh"), !c.can("refresh")),
          button("取消操作", () => action("cancel"), !c.can("cancel")),
          button(
            c.presentation.template === "OperationApproval" || s.fallbackReason
              ? "确认操作"
              : "保存",
            () => action("submit"),
            !c.can("submit") ||
              (c.presentation.template === "GuidedFlow" &&
                !s.fallbackReason &&
                s.step < (c.presentation.steps?.length ?? 1) - 1),
          ),
        ]);
      throw new Error("未知操作组件");
    };
  },
});
export const mountNode: NodeMount = (container, root, enabled) => {
  const app = createApp(
    defineComponent({
      setup() {
        return () =>
          h("div", { class: "rss-a2ui-surface", inert: !enabled }, [
            h(NodeView, { node: root }),
          ]);
      },
    }),
  );
  app.config.errorHandler = (error) => {
    throw error;
  };
  app.mount(container);
  return () => app.unmount();
};
