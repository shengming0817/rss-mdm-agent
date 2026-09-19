import type { ClientErrorCode } from "@rss-mdm-agent/ai-client";
const messages = {
  renderer_disposed: "This card has been closed.",
  renderer_load:
    "Card renderer could not load. Text and standard permissions remain available.",
  renderer_invalid:
    "This card cannot be displayed. Use Retry card to reload it.",
  action_failed:
    "Response could not be confirmed. Retry the response or restore the session.",
  action_rejected:
    "This question can no longer accept this response. Restore the session.",
} as const;
export type RendererErrorCode = keyof typeof messages;
/** Fixed messages and closed failure codes; never stores model content or raw causes. */
export class RendererError extends Error {
  constructor(
    readonly code: RendererErrorCode,
    readonly failure?: ClientErrorCode,
  ) {
    super(messages[code]);
    this.name = "RendererError";
  }
}
