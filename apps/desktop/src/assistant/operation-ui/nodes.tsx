import {
  Component,
  useCallback,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { createRoot, type Root } from "react-dom/client";
import { Button, Input, Alert } from "antd";
import {
  effect,
  getValue,
  isComponentNode,
  isWritable,
  ResolvedBinding,
  type ComponentNode,
} from "@a2ui/web_core/v0_9";
import type {
  NodeGate,
  NodeMount,
  OperationController,
} from "@rss-mdm-agent/ai-ui-bridge";
import { OperationNode } from "./OperationNode";

export const nodeValue = (value: unknown) =>
  value instanceof ResolvedBinding ? value.value : value;
export function nodeText(value: unknown, max = 8192): string {
  const plain = nodeValue(value);
  return typeof plain === "string"
    ? plain.slice(0, max)
    : typeof plain === "number" || typeof plain === "boolean"
      ? String(plain)
      : "";
}
const readonlyGate: NodeGate = {
  snapshot: () => 0,
  subscribe: () => () => {},
  canEdit: () => false,
  reason: () => "",
};
export type NodeContext =
  | { kind: "question"; gate: NodeGate }
  | { kind: "operation"; controller: OperationController };

/** Subscribe through official public signals; each resolved node keeps its own identity. */
export function NodeView({
  node,
  context,
}: {
  node: ComponentNode;
  context: NodeContext;
}) {
  const subscribe = useCallback(
      (notify: () => void) =>
        effect(() => {
          getValue(node.props);
          notify();
        }),
      [node],
    ),
    read = useCallback(() => getValue(node.props), [node]),
    props = useSyncExternalStore(subscribe, read),
    gate = context.kind === "question" ? context.gate : readonlyGate;
  useSyncExternalStore(gate.subscribe, gate.snapshot);
  const child = (value: unknown): ReactNode =>
      isComponentNode(value) ? (
        <NodeView key={value.instanceId} node={value} context={context} />
      ) : null,
    children = (Array.isArray(props.children) ? props.children : []).map(child);
  if (node.disposed || node.isPlaceholder)
    return <p className="rss-muted">这部分内容暂不可用。</p>;
  if (context.kind === "operation")
    return (
      <OperationNode
        node={node}
        props={props}
        controller={context.controller}
        renderChild={child}
        children={children}
      />
    );
  const enabled = gate.canEdit();
  switch (node.type) {
    case "Column":
      return <div className="rss-ai-fields">{children}</div>;
    case "Text":
      return <p className="rss-ai-text">{nodeText(props.text)}</p>;
    case "Button":
      return (
        <Button
          type="primary"
          disabled={!enabled || typeof props.action !== "function"}
          onClick={() => {
            if (
              !node.disposed &&
              gate.canEdit() &&
              typeof props.action === "function"
            )
              props.action();
          }}
        >
          {child(props.child) || nodeText(props.label) || "提交回答"}
        </Button>
      );
    case "TextField": {
      const binding = props.value,
        protectedField = nodeText(props.variant) === "obscured",
        writable = binding instanceof ResolvedBinding && isWritable(binding),
        value = protectedField ? "" : nodeText(binding),
        edit = (value: string) => {
          if (
            !node.disposed &&
            gate.canEdit() &&
            !protectedField &&
            binding instanceof ResolvedBinding &&
            isWritable(binding)
          )
            binding.set(value);
        };
      return (
        <label className="rss-ai-field">
          <span>{nodeText(props.label, 300)}</span>
          {nodeText(props.variant) === "longText" ? (
            <Input.TextArea
              aria-label={nodeText(props.label, 300)}
              value={value}
              disabled={!enabled || !writable}
              maxLength={8192}
              onChange={(event) => edit(event.target.value)}
            />
          ) : (
            <Input
              aria-label={nodeText(props.label, 300)}
              value={value}
              disabled={!enabled || !writable || protectedField}
              maxLength={8192}
              onChange={(event) => edit(event.target.value)}
            />
          )}
        </label>
      );
    }
    default:
      throw Error("unsupported resolved question component");
  }
}
export class CardBoundary extends Component<
  { children: ReactNode; fallback: ReactNode; onError(error: unknown): void },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(error: unknown) {
    this.props.onError(error);
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}
function QuestionRoot({
  node,
  enabled,
  gate,
}: {
  node: ComponentNode;
  enabled: boolean;
  gate: NodeGate;
}) {
  useSyncExternalStore(gate.subscribe, gate.snapshot);
  const effective: NodeGate = enabled
    ? gate
    : { ...gate, canEdit: () => false };
  return (
    <div className="rss-question-surface">
      {gate.reason() && <Alert type="warning" title={gate.reason()} />}
      <NodeView node={node} context={{ kind: "question", gate: effective }} />
    </div>
  );
}
const roots = new WeakMap<HTMLElement, { root: Root; lease: number }>();
/** One DOM owner survives effect replacements; retirement fences actions before deferred unmount. */
export const mountQuestionNode: NodeMount = (container, onError) => {
  let entry = roots.get(container);
  if (!entry) {
    entry = { root: createRoot(container), lease: 0 };
    roots.set(container, entry);
  }
  const owned = entry,
    lease = ++owned.lease;
  container.inert = false;
  let alive = true,
    failed = false;
  const report = (error: unknown) => {
    failed = true;
    onError(error);
  };
  return {
    render(node, enabled, gate) {
      if (!alive || owned.lease !== lease) return;
      if (failed) throw Error("question renderer failed");
      owned.root.render(
        node ? (
          <CardBoundary
            key={lease}
            onError={report}
            fallback={
              <p role="status">问题表单未能显示，原问题与对话文本仍可读取。</p>
            }
          >
            <QuestionRoot node={node} enabled={enabled} gate={gate} />
          </CardBoundary>
        ) : null,
      );
    },
    dispose() {
      if (!alive) return;
      alive = false;
      container.inert = true;
      void Promise.resolve().then(() => {
        if (owned.lease !== lease) return;
        owned.root.unmount();
        roots.delete(container);
      });
    },
  };
};
