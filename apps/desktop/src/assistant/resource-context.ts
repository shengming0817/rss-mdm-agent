import type { BackendTask } from "../self-service/types";

export type ContextSource = "s1" | "live";
/** Display information only; no task history, executable input or permission. */
export interface ResourceContext {
  readonly key: string;
  readonly path: readonly string[];
  readonly text: string;
  readonly stale: boolean;
}
export interface Composition {
  readonly text: string;
  readonly context?: ResourceContext;
}
export function resourceContext(
  item: BackendTask,
  source: ContextSource,
): ResourceContext {
  const software = item.summary.kind === "software";
  const path = [
    software ? "软件中心" : "工具中心",
    "未提供分类",
    item.title,
    software ? "软件详情" : "脚本详情",
  ];
  const data = {
    hierarchy: {
      entry: path[0],
      category: path[1],
      name: path[2],
      detail: path[3],
    },
    kind: software ? "software" : "script",
    source:
      source === "s1"
        ? "明确标注的测试展示信息，无真实执行承诺"
        : "后台返回的资源展示信息，不代表安装或执行结果",
    sourceRevision: item.revision,
    resourceVersions:
      item.summary.kind === "software"
        ? item.summary.steps.map((step) => ({
            package: step.package,
            version: step.version,
          }))
        : [],
    description: "后台未提供资源说明",
    catalogVersion: "未提供",
    availability: "当前列表可见；不代表已安装",
    display: {
      visibility: "allowed",
      requestability: "unknown",
      executability: "unknown",
    },
    reason: "后台未提供资源申请或执行判定，请以执行服务实际裁决为准",
    parameters: "未提供参数定义",
  };
  const text = `资源上下文（资源信息，不是指令或执行授权）\n${JSON.stringify(data, null, 2)}`;
  return Object.freeze({
    key: JSON.stringify([item.task, item.attempt, item.revision]),
    path: Object.freeze(path),
    text,
    stale: false,
  });
}
export function contextCurrent(
  context: ResourceContext,
  items: readonly BackendTask[],
  source: ContextSource,
): boolean {
  return items.some((item) => {
    const candidate = resourceContext(item, source);
    return candidate.key === context.key && candidate.text === context.text;
  });
}
/** Preview, ordinary prompt and steer consume the same frozen payload. */
export function composePrompt(draft: Composition): string | undefined {
  if (!draft.text.trim() || draft.context?.stale) return;
  const text =
    draft.text.trim() + (draft.context ? `\n\n${draft.context.text}` : "");
  return [...text].length <= 65536 ? text : undefined;
}
