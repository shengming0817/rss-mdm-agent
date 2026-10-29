import type { ClientErrorCode } from "@rss-mdm-agent/ai-client";
const messages = {
  renderer_disposed: "这张卡片已关闭。",
  renderer_load: "互动卡片未能加载；对话文本和原问题状态仍可读取。",
  renderer_invalid: "这张互动卡片暂时无法显示，可重新加载卡片。",
  action_failed: "原回答回执未能确认，请读取原问题状态或重试同一回答。",
  action_rejected: "原问题不能再接受此回答，请读取原会话。",
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
