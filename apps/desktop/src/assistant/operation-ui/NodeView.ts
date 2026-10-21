import {
  createApp,
  defineComponent,
  h,
  inject,
  provide,
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
import {
  UiProvider,
  UiText,
  UiStack,
  UiButton,
  UiField,
  UiTextInput,
  UiNumberInput,
  UiSelect,
  UiCheckbox,
  UiSwitch,
  UiRadioGroup,
  UiDateTimeInput,
  UiTabs,
  UiCollapsible,
  UiTable,
  UiProgress,
  UiSteps,
  UiMessage,
  type UiFocusable,
  type UiConfiguration,
} from "@rss-mdm-agent/ui";
import "./style.css";
const focusRegistry: InjectionKey<Map<string, () => Promise<void>>> =
  Symbol("operation-focus");
const revealContext: InjectionKey<(child: string) => Promise<void>> =
  Symbol("operation-reveal");
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
  execution_ended: "执行结束，等待核实",
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
    const focuses = inject(
      focusRegistry,
      new Map<string, () => Promise<void>>(),
    );
    provide(focusRegistry, focuses);
    const fieldControl = ref<UiFocusable>();
    const disclosureOpen = ref(props.node.type === "Disclosure");
    const revealParent = inject(revealContext, async (_child: string) => {});
    const fieldId = props.node.componentId.startsWith("field-")
      ? props.node.componentId.slice(6)
      : "";
    const focusField = async () => {
      await revealParent(props.node.instanceId);
      await nextTick();
      fieldControl.value?.focus();
    };
    if (fieldId) focuses.set(fieldId, focusField);
    onBeforeUnmount(() => {
      if (focuses.get(fieldId) === focusField) focuses.delete(fieldId);
    });
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
    provide(revealContext, async (childId) => {
      await revealParent(props.node.instanceId);
      if (props.node.type === "Disclosure") disclosureOpen.value = true;
      if (props.node.type === "Tabs") {
        const index = ((p.value.children ?? []) as ComponentNode[]).findIndex(
          (node) => node.instanceId === childId,
        );
        if (index >= 0) selectedTab.value = index;
      }
      await nextTick();
    });
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
      h(UiButton, { disabled, onClick: action }, () => label);
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
      await nextTick();
      await focuses.get(id)?.();
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
      const type = c?.isSecret(fieldId) ? "SecretField" : n.type,
        s = c?.state;
      if (type === "Text")
        return h(UiText, {}, () => display(valueOf(propsValue.text)));
      if (type === "Column" || type === "FieldGroup")
        return h(
          type === "FieldGroup" ? "fieldset" : "div",
          { class: "operation-fields" },
          children(),
        );
      if (type === "Button")
        return h(
          UiButton,
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
          {
            class: "operation-surface",
            "aria-label": s?.snapshot.presentation.title,
          },
          children(),
        );
      if (type === "Section")
        return h("section", { class: "operation-section" }, [
          h("h3", display(propsValue.label)),
          ...children(),
        ]);
      if (type === "Disclosure")
        return h(
          UiCollapsible,
          {
            open: disclosureOpen.value,
            label: display(propsValue.label),
            "onUpdate:open": (open: boolean) => (disclosureOpen.value = open),
          },
          children,
        );
      if (type === "Tabs") {
        const nodes = (propsValue.children ?? []) as ComponentNode[];
        return h(
          UiTabs,
          {
            modelValue: nodes[selectedTab.value]?.instanceId ?? "",
            label: "设置分组",
            items: nodes.map((node) => ({
              id: node.instanceId,
              label: display(getValue(node.props).label),
            })),
            "onUpdate:modelValue": (id: string) =>
              (selectedTab.value = nodes.findIndex(
                (node) => node.instanceId === id,
              )),
          },
          {
            default: ({ item }: { item: { id: string } }) =>
              child(nodes.find((node) => node.instanceId === item.id)!),
          },
        );
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
        const secret = c ? c.isSecret(fieldId) : type === "SecretField";
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
        const attrs = { disabled, required: !!field?.required };
        const label = field?.label ?? display(valueOf(propsValue.label));
        const options = (field?.options ?? []).map((o) => ({
          value: o.id,
          label: o.disabledReason
            ? `${o.label}（${o.disabledReason}）`
            : o.label,
          disabled: !!o.disabledReason,
        }));
        let input: VNodeChild;
        if (type === "RadioGroup")
          input = h(UiRadioGroup, {
            ...attrs,
            label,
            options,
            modelValue: String(current ?? ""),
            "onUpdate:modelValue": write,
          });
        else if (type === "SelectField")
          input = h(UiSelect, {
            ...attrs,
            options,
            modelValue: String(current ?? ""),
            "onUpdate:modelValue": write,
          });
        else if (type === "Checkbox")
          input = h(UiCheckbox, {
            ...attrs,
            modelValue: current === true,
            "onUpdate:modelValue": (value: boolean | "indeterminate") =>
              write(value === true),
          });
        else if (type === "Switch")
          input = h(UiSwitch, {
            ...attrs,
            modelValue: current === true,
            "onUpdate:modelValue": write,
          });
        else if (type === "NumberField" || variant === "number")
          input = h(UiNumberInput, {
            ...attrs,
            modelValue:
              typeof current === "number"
                ? current
                : current !== "" && Number.isFinite(Number(current))
                  ? Number(current)
                  : null,
            min: field?.min,
            max: field?.max,
            "onUpdate:modelValue": (value: number | null) =>
              write(
                type === "NumberField"
                  ? value
                  : value === null
                    ? ""
                    : String(value),
              ),
          });
        else if (type === "DateTimeField")
          input = h(UiDateTimeInput, {
            ...attrs,
            modelValue: String(current ?? ""),
            "onUpdate:modelValue": write,
          });
        else
          input = h(UiTextInput, {
            ...attrs,
            modelValue: secret ? "" : String(current ?? ""),
            multiline: variant === "longText",
            type: secret || variant === "obscured" ? "password" : "text",
            autocomplete: secret ? "off" : undefined,
            maxlength: field?.maxLength ?? 8192,
            "onUpdate:modelValue": (value: string) => {
              if (!secret) write(value);
            },
            onChange: secret
              ? (event: Event) => {
                  const el = event.target as HTMLInputElement;
                  const value = el.value;
                  el.value = "";
                  void c?.secret(fieldId, value);
                }
              : undefined,
          });
        const description =
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
                  : "");
        return h(
          UiField,
          {
            ref: fieldControl,
            id: domId,
            class: "operation-field",
            "data-operation-field": field?.id,
            label,
            error,
            description,
            required: !!field?.required,
          },
          () => [
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
          h("dd", snapshot.presentation.title),
          h("dt", "来源"),
          h("dd", snapshot.presentation.source),
          h("dt", "目标"),
          h("dd", snapshot.presentation.targetLabel),
          h("dt", "影响"),
          h("dd", snapshot.presentation.impact),
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
        return h(UiTable, {
          label: "操作对象",
          columns: [
            { key: "label", label: "对象" },
            { key: "detail", label: "详情" },
            { key: "status", label: "状态" },
          ],
          rows: snapshot.objects.map((o) => ({
            ...o,
            disabled: !!o.disabledReason,
            status: o.disabledReason || o.status || "可用",
          })),
          selection:
            c.presentation.selection === "single" ? "single" : "multiple",
          modelValue: s.draft.selection,
          disabled: !c.can("submit"),
          "onUpdate:modelValue": (ids: string[]) => {
            const changed = [...new Set([...ids, ...s.draft.selection])].find(
              (id) => ids.includes(id) !== s.draft.selection.includes(id),
            );
            if (changed)
              c.select(
                c.presentation.selection === "single" ? ids[0] : changed,
              );
          },
        });
      if (type === "ChangePreview")
        return h(UiTable, {
          label: "修改预览",
          columns: [
            { key: "label", label: "参数" },
            { key: "before", label: "当前" },
            { key: "after", label: "提交值" },
          ],
          rows: Object.entries(s.draft.values).map(([id, value]) => ({
            id,
            label: c.presentation.fields.find((f) => f.id === id)?.label ?? id,
            before: display(snapshot.current[id]),
            after: display(value),
          })),
        });
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
            ? h(UiProgress, {
                value: snapshot.progress.completed,
                max: snapshot.progress.total,
                label: "任务进度",
              })
            : null,
        ]);
      if (type === "ResultSummary")
        return snapshot.result
          ? h("p", { role: "status" }, snapshot.result)
          : null;
      if (type === "LogView")
        return h(
          UiCollapsible,
          {
            open: disclosureOpen.value,
            label: "操作日志",
            "onUpdate:open": (open: boolean) => (disclosureOpen.value = open),
          },
          () =>
            h(
              "pre",
              { role: "log", "aria-label": "操作日志" },
              snapshot.logs.join("\n"),
            ),
        );
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
          h(UiSteps, {
            items: c.presentation.steps ?? [],
            current: c.presentation.steps?.[s.step]?.id ?? "",
            label: "操作步骤",
          }),
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
export const createNodeMount =
  (configuration: Readonly<UiConfiguration>): NodeMount =>
  (container, root, enabled) => {
    const app = createApp(
      defineComponent({
        setup() {
          return () =>
            h(UiProvider, configuration, () =>
              h("div", { class: "rss-a2ui-surface", inert: !enabled }, [
                h(NodeView, { node: root }),
              ]),
            );
        },
      }),
    );
    app.config.errorHandler = (error) => {
      throw error;
    };
    app.mount(container);
    return () => app.unmount();
  };
