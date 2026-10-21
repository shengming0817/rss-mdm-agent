import { boundedJson } from "./codec.js";
import { defaultLimits } from "./results.js";
import validatePresentation from "./validate-operation-presentation.js";
import validateSnapshot from "./validate-operation-snapshot.js";
import validateEvent from "./validate-operation-event.js";
import type {
  OperationPresentation,
  OperationSnapshot,
  OperationEvent,
  OperationScope,
} from "./operation-types.js";
export type * from "./operation-types.js";
export { operationCatalog } from "./operation-catalog.js";

export class OperationInputError extends Error {
  constructor() {
    super("操作界面数据无效或版本不支持");
  }
}
/** Definite native-service rejection. Transport ambiguity must never be wrapped as this error. */
export class OperationRejectedError extends Error {
  constructor(message: string) {
    super(message);
  }
}
function bounded<T>(input: unknown, validate: (input: unknown) => boolean): T {
  let raw: string;
  try {
    raw = boundedJson(input, { ...defaultLimits, maxDepth: 24 });
  } catch {
    throw new OperationInputError();
  }
  if (!raw || new TextEncoder().encode(raw).length > 262144 || !validate(input))
    throw new OperationInputError();
  const scan = (value: unknown, depth: number): void => {
    if (depth > 24) throw new OperationInputError();
    if (value && typeof value === "object")
      for (const [key, child] of Object.entries(value)) {
        if (["__proto__", "prototype", "constructor"].includes(key))
          throw new OperationInputError();
        scan(child, depth + 1);
      }
  };
  scan(input, 0);
  return JSON.parse(raw) as T;
}
export function operationPresentation(input: unknown): OperationPresentation {
  const value = bounded<OperationPresentation>(input, validatePresentation);
  if (value.template === "GuidedFlow" && !value.steps?.length)
    throw new OperationInputError();
  const ids = new Set(value.fields.map((f) => f.id));
  const sections = new Set(value.sections.map((s) => s.id));
  if (
    ids.size !== value.fields.length ||
    sections.size !== value.sections.length ||
    value.fields.some(
      (f) =>
        ["__proto__", "prototype", "constructor"].includes(f.id) ||
        (f.section && !sections.has(f.section)) ||
        (f.min !== undefined && f.max !== undefined && f.min > f.max) ||
        (f.component === "Switch" && !f.submitMode) ||
        (f.options &&
          new Set(f.options.map((o) => o.id)).size !== f.options.length),
    ) ||
    value.rules.some((r) => !ids.has(r.field) || !ids.has(r.when)) ||
    value.steps?.some((s) => s.fields.some((id) => !ids.has(id)))
  )
    throw new OperationInputError();
  return value;
}
export const operationSnapshot = (input: unknown): OperationSnapshot =>
  bounded(input, validateSnapshot);
export const operationEvent = (input: unknown): OperationEvent =>
  bounded(input, validateEvent);
/** In-process behavior port. Only the application composition root supplies it.
 * Snapshots are service projections, never AI presentation messages. No execution authority lives here. */
export interface OperationHostPort {
  read(scope: OperationScope): Promise<OperationSnapshot>;
  dispatch(
    event: OperationEvent,
    scope: OperationScope,
  ): Promise<OperationSnapshot>;
  observe(
    scope: OperationScope,
    update: (observation: {
      snapshot: OperationSnapshot;
      kind: "query" | "projection";
    }) => void,
  ): () => void;
  choosePath?(
    scope: OperationScope,
    field: string,
  ): Promise<string | undefined>;
  captureSecret?(
    scope: OperationScope,
    field: string,
    value: string,
  ): Promise<string>;
  revokeSecret?(reference: string): void;
}
