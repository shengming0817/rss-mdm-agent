/** Display information only; no task history, executable input or permission. */
export interface ResourceContext {
  readonly scope: "page";
  readonly key: string;
  readonly path: readonly string[];
  readonly text: string;
  readonly stale: boolean;
  readonly ownerKey: string;
  readonly expiresAt: number;
}
export interface Composition {
  readonly text: string;
  readonly context?: ResourceContext;
}
/** Preview, ordinary prompt and steer consume the same frozen payload. */
export function composePrompt(
  draft: Composition,
  now = Date.now(),
): string | undefined {
  if (
    !draft.text.trim() ||
    draft.context?.stale ||
    (draft.context && draft.context.expiresAt <= now)
  )
    return;
  const text =
    draft.text.trim() + (draft.context ? `\n\n${draft.context.text}` : "");
  return [...text].length <= 65536 ? text : undefined;
}
