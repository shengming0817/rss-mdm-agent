import type { Budget } from "@rss-mdm-agent/ai-contract";
import type { DeepSeekDiagnostic } from "./configuration.js";
export class NativeFault extends Error {
  constructor(readonly reason: DeepSeekDiagnostic["reason"]) {
    super(reason);
  }
}
export const diagnosticReasons: readonly DeepSeekDiagnostic["reason"][] = [
  "configuration_rejected",
  "dependency_drift",
  "restoration_failed",
  "native_failure",
  "protocol_failure",
  "spawn_failed",
  "process_exit",
  "profile_drift",
  "cleanup_failed",
];
export type Operation =
  | "initialize"
  | "prompt"
  | "inspect"
  | "cancel"
  | "answer"
  | "tool_result"
  | "close";
export interface NativeEvent {
  type:
    | "accepted"
    | "delta"
    | "text"
    | "terminal"
    | "question"
    | "question_unavailable"
    | "proposal"
    | "lost"
    | "error";
  diagnostic?: Omit<DeepSeekDiagnostic, "generation">;
  requestId?: string;
  messageId?: string;
  text?: string;
  outcome?:
    | "completed"
    | "refused"
    | "cancelled"
    | "failed"
    | "max_tokens"
    | "max_turn_requests";
  callbackId?: string;
  request?: Record<string, unknown>;
  proposal?: { name: string; arguments: Record<string, unknown> };
}
export interface NativeRuntime {
  call(operation: Operation, value: unknown, budget: Budget): Promise<any>;
  onEvent(listener: (event: NativeEvent) => void): void;
  stop(): void;
  readonly stopped: Promise<void>;
}
export type RuntimeFactory = () => NativeRuntime;
