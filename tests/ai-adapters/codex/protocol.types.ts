import type { NativeNotification } from "../../../packages/ai-adapters/codex/dist/protocol.js";
import type { AgentMessageDeltaNotification } from "../../../packages/ai-adapters/codex/dist/protocol/v2/AgentMessageDeltaNotification.js";
import type { TurnCompletedNotification } from "../../../packages/ai-adapters/codex/dist/protocol/v2/TurnCompletedNotification.js";
import type { ItemCompletedNotification } from "../../../packages/ai-adapters/codex/dist/protocol/v2/ItemCompletedNotification.js";
import type { ItemStartedNotification } from "../../../packages/ai-adapters/codex/dist/protocol/v2/ItemStartedNotification.js";

declare const notification: NativeNotification;
switch (notification.method) {
  case "item/agentMessage/delta": {
    const params: AgentMessageDeltaNotification = notification.params;
    const delta: string = params.delta;
    // @ts-expect-error A delta is not a completed turn; any would hide protocol drift.
    notification.params.turn;
    break;
  }
  case "turn/completed": {
    const params: TurnCompletedNotification = notification.params;
    // @ts-expect-error turn/completed owns a Turn, not the delta payload.
    notification.params.delta;
    break;
  }
  case "item/completed": {
    const params: ItemCompletedNotification = notification.params;
    const timestamp: number = params.completedAtMs;
    // @ts-expect-error Lifecycle payloads do not own text deltas.
    notification.params.delta;
    break;
  }
  case "item/started": {
    const params: ItemStartedNotification = notification.params;
    // @ts-expect-error Lifecycle payloads do not own text deltas.
    notification.params.delta;
    break;
  }
  default: {
    const exhaustive: never = notification;
  }
}

import {
  createCodexAdapter,
  type CodexAdapterOptions,
  type ResolvedCodexConfiguration,
} from "../../../packages/ai-adapters/codex/dist/index.js";
import type {
  ProviderAgentPort,
  ProviderDiagnostic,
} from "../../../packages/ai-contract/dist/index.js";
declare const options: CodexAdapterOptions;
const port: ProviderAgentPort = createCodexAdapter(options).agent;
// @ts-expect-error Native history is not a public provider operation.
port.readHistory;
// @ts-expect-error Host admission is not a provider operation.
port.fork;
const unsafeDiagnostic: ProviderDiagnostic = {
  kind: "other",
  dropped: 0,
  // @ts-expect-error Diagnostics expose no business payload.
  message: {},
};
// @ts-expect-error A Codex resolver cannot claim another provider.
const wrongProvider: ResolvedCodexConfiguration["configuration"]["provider"] =
  "claude";
createCodexAdapter({
  resolveConfiguration: options.resolveConfiguration,
  // @ts-expect-error Native binary and arbitrary app-server options are sealed.
  binaryPath: "/fixture/codex",
});
