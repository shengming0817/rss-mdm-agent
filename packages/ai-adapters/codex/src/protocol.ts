import type { InitializeParams } from "./protocol/InitializeParams.js";
import type { InitializeResponse } from "./protocol/InitializeResponse.js";
import type { ThreadStartParams } from "./protocol/v2/ThreadStartParams.js";
import type { ThreadStartResponse } from "./protocol/v2/ThreadStartResponse.js";
import type { ThreadResumeParams } from "./protocol/v2/ThreadResumeParams.js";
import type { ThreadResumeResponse } from "./protocol/v2/ThreadResumeResponse.js";
import type { ThreadForkParams } from "./protocol/v2/ThreadForkParams.js";
import type { ThreadForkResponse } from "./protocol/v2/ThreadForkResponse.js";
import type { ThreadReadParams } from "./protocol/v2/ThreadReadParams.js";
import type { ThreadReadResponse } from "./protocol/v2/ThreadReadResponse.js";
import type { ThreadTurnsListParams } from "./protocol/v2/ThreadTurnsListParams.js";
import type { ThreadTurnsListResponse } from "./protocol/v2/ThreadTurnsListResponse.js";
import type { TurnStartParams } from "./protocol/v2/TurnStartParams.js";
import type { TurnStartResponse } from "./protocol/v2/TurnStartResponse.js";
import type { TurnSteerParams } from "./protocol/v2/TurnSteerParams.js";
import type { TurnSteerResponse } from "./protocol/v2/TurnSteerResponse.js";
import type { TurnInterruptParams } from "./protocol/v2/TurnInterruptParams.js";
import type { ListMcpServerStatusParams } from "./protocol/v2/ListMcpServerStatusParams.js";
import type { ListMcpServerStatusResponse } from "./protocol/v2/ListMcpServerStatusResponse.js";
import type { ConfigReadParams } from "./protocol/v2/ConfigReadParams.js";
import type { ConfigReadResponse } from "./protocol/v2/ConfigReadResponse.js";
import type { Budget } from "@rss-mdm-agent/ai-contract";
import type { RpcConnection } from "./runtime.js";
export type { Thread } from "./protocol/v2/Thread.js";
export type { Turn } from "./protocol/v2/Turn.js";
export type { ThreadItem } from "./protocol/v2/ThreadItem.js";
interface Methods {
  initialize: [InitializeParams, InitializeResponse];
  "thread/start": [ThreadStartParams, ThreadStartResponse];
  "thread/resume": [ThreadResumeParams, ThreadResumeResponse];
  "thread/fork": [ThreadForkParams, ThreadForkResponse];
  "thread/read": [ThreadReadParams, ThreadReadResponse];
  "thread/turns/list": [ThreadTurnsListParams, ThreadTurnsListResponse];
  "turn/start": [TurnStartParams, TurnStartResponse];
  "turn/steer": [TurnSteerParams, TurnSteerResponse];
  "turn/interrupt": [TurnInterruptParams, Record<string, never>];
  "mcpServerStatus/list": [
    ListMcpServerStatusParams,
    ListMcpServerStatusResponse,
  ];
  "config/read": [ConfigReadParams, ConfigReadResponse];
}
/** Generated types own protocol shapes; adapter validates consumed response fields at each boundary. */
export async function rpc<K extends keyof Methods>(
  connection: RpcConnection,
  method: K,
  params: Methods[K][0],
  budget: Budget,
): Promise<Methods[K][1]> {
  return (await connection.request(method, params, budget)) as Methods[K][1];
}
