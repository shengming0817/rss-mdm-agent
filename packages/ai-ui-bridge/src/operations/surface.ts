import {
  Catalog,
  MessageProcessor,
  NodeResolver,
  type ComponentNode,
  type Signal,
  type A2uiMessage,
} from "@a2ui/web_core/v0_9";
import {
  operationCatalog,
  type OperationPresentation,
} from "@rss-mdm-agent/ai-contract/operations";

const loaded = Catalog.fromSchema(operationCatalog);
const catalog = new Catalog(loaded.id, [...loaded.components.values()], []);
/** Fixed product templates only; callers never submit arbitrary component trees. */
export function operationComponents(
  p: OperationPresentation,
  fallback = false,
): Record<string, unknown>[] {
  const nodes: Record<string, unknown>[] = [];
  const add = (
    id: string,
    component: string,
    props: Record<string, unknown> = {},
  ) => {
    nodes.push({ id, component, ...props });
    return id;
  };
  const children = [
    add("notice", "Notice"),
    add("summary", "ReadonlyRow"),
    add("approval", "ApprovalStatus"),
  ];
  const template = fallback ? "OperationApproval" : p.template;
  if (["ObjectActions", "ResourceSelection"].includes(template))
    children.push(
      add(
        "objects",
        template === "ObjectActions" ? "ObjectList" : "SelectableTable",
      ),
    );
  if (template !== "TaskExecution") {
    const sections = p.sections.length
      ? p.sections
      : [{ id: "default", label: "参数" }];
    const groups = sections.map((section, i) => {
      const fields = p.fields
        .filter((f) => (f.section ? f.section === section.id : i === 0))
        .map((f) =>
          add(
            `field-${f.id}`,
            f.component,
            f.component === "SecretField"
              ? { label: f.label, fieldId: f.id }
              : f.component === "TextField"
                ? { label: f.label, value: { path: `/draft/${f.id}` } }
                : {
                    label: f.label,
                    fieldId: f.id,
                    value: { path: `/draft/${f.id}` },
                  },
          ),
        );
      return add(
        `section-${section.id}`,
        !fallback && p.layout === "disclosure" ? "Disclosure" : "Section",
        {
          label: section.label,
          children: [
            add(`group-${section.id}`, "FieldGroup", { children: fields }),
          ],
        },
      );
    });
    children.push(
      ...(!fallback && p.layout === "tabs"
        ? [add("tabs", "Tabs", { children: groups })]
        : groups),
    );
  }
  if (template === "GuidedFlow") children.unshift(add("steps", "StepFlow"));
  if (
    [
      "SettingsForm",
      "ChangeReview",
      "OperationApproval",
      "GuidedFlow",
    ].includes(template)
  )
    children.push(add("changes", "ChangePreview"));
  if (["ChangeReview", "OperationApproval"].includes(template))
    children.push(add("confirmation", "ConfirmationPanel"));
  children.push(
    add("waiting", "PendingInteraction"),
    add("progress", "Progress"),
    add("result", "ResultSummary"),
    add("logs", "LogView"),
    add("actions", "ActionBar"),
  );
  add("root", "OperationSurface", { children });
  return nodes;
}
export function createOperationSurface(
  presentation: OperationPresentation,
  fallback = false,
): {
  root: Signal<ComponentNode | undefined>;
  update(draft: Record<string, unknown>): void;
  dispose(): void;
} {
  const id = "operation";
  const processor = new MessageProcessor([catalog], async () => {}, {
    version: "v0.9.1",
  });
  processor.processMessages([
    {
      version: "v0.9.1",
      createSurface: { surfaceId: id, catalogId: catalog.id },
    },
    {
      version: "v0.9.1",
      updateComponents: {
        surfaceId: id,
        components: operationComponents(presentation, fallback),
      },
    },
  ] as unknown as A2uiMessage[]);
  const model = processor.model.getSurface(id)!;
  const resolver = new NodeResolver(model, catalog);
  return {
    root: resolver.rootNode,
    update(draft: Record<string, unknown>) {
      model.dataModel.set("/draft", JSON.parse(JSON.stringify(draft)));
    },
    dispose() {
      resolver.dispose();
      processor.model.dispose();
    },
  };
}
/** One React DOM owner; subsequent official roots update the existing view. */
export type NodeMount = (
  container: HTMLElement,
  onError: (error: unknown) => void,
) => {
  render(
    root: ComponentNode | undefined,
    enabled: boolean,
    gate: import("../surface-drafts.js").NodeGate,
  ): void;
  dispose(): void;
};
