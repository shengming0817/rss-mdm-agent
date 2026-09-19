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
  "invalid_input",
  "interaction_unavailable",
  "budget_exhausted",
];
export interface Initialization {
  nativeSessionId: string;
  workingDirectory: string;
  persistenceDirectory: string;
  scope: string;
  model: string;
  apiKey: string;
  apiUrl: string;
  controlled: boolean;
  restore: boolean;
  composition: string;
  previousRequestId?: string;
}
export type QuestionAnswer = {
  answers: { id: string; selected: string[]; custom?: string }[];
};
export type QuestionRequest = {
  questions: {
    id: string;
    question: string;
    detail?: string;
    header?: string;
    options?: { label: string; description?: string }[];
    multiSelect?: boolean;
  }[];
};
export type ToolResult = {
  disposition: "returned" | "rejected" | "unavailable";
  text: string;
};
export type NativeOutcome =
  | "completed"
  | "refused"
  | "cancelled"
  | "failed"
  | "max_tokens"
  | "max_turn_requests";
export type Inspection =
  | { status: "unknown" | "running" }
  | { status: "terminal"; outcome: NativeOutcome };
export interface OperationRequest {
  initialize: Initialization;
  prompt: { requestId: string; text: string };
  inspect: { requestId: string };
  cancel: Record<string, never>;
  answer: { callbackId: string } & (
    | { unavailable: true }
    | { answer: QuestionAnswer }
  );
  tool_result: { callbackId: string; value: ToolResult };
  close: Record<string, never>;
}
export interface OperationResponse {
  initialize: {
    composition: string;
    nativeSessionId: string;
    activation: string;
    observationOnly: boolean;
    previousTerminal: boolean;
  };
  prompt: {
    status: "accepted";
    requestId: string;
    activation: string;
    observationOnly: false;
  };
  inspect: Inspection;
  cancel: Record<string, never>;
  answer: Record<string, never>;
  tool_result: Record<string, never>;
  close: Record<string, never>;
}
export type Operation = keyof OperationRequest;
export type RequestFrame = {
  [K in Operation]: { id: number; operation: K; value: OperationRequest[K] };
}[Operation];
export type OperationHandlers = {
  [K in Operation]: (
    value: OperationRequest[K],
  ) => OperationResponse[K] | Promise<OperationResponse[K]>;
};
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
  request?: QuestionRequest;
  proposal?: { name: string; arguments: Record<string, unknown> };
}
export interface NativeRuntime {
  call<K extends Operation>(
    operation: K,
    value: OperationRequest[K],
    budget: Budget,
  ): Promise<OperationResponse[K]>;
  onEvent(listener: (event: NativeEvent) => void): void;
  stop(): void;
  readonly stopped: Promise<void>;
}
export type RuntimeFactory = () => NativeRuntime;

const outcomes: readonly NativeOutcome[] = [
  "completed",
  "refused",
  "cancelled",
  "failed",
  "max_tokens",
  "max_turn_requests",
];
const operations: readonly Operation[] = [
  "initialize",
  "prompt",
  "inspect",
  "cancel",
  "answer",
  "tool_result",
  "close",
];
export function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function check(condition: unknown): asserts condition {
  if (!condition) throw new NativeFault("protocol_failure");
}
const fields = (
  v: Record<string, unknown>,
  required: string[],
  optional: string[] = [],
) =>
  required.every((k) => Object.hasOwn(v, k)) &&
  Object.keys(v).every((k) => required.includes(k) || optional.includes(k));
const strings = (v: Record<string, unknown>, keys: string[]) =>
  keys.every((k) => typeof v[k] === "string");
export function decodeAnswer(value: unknown): QuestionAnswer {
  if (
    !record(value) ||
    !fields(value, ["answers"]) ||
    !Array.isArray(value.answers) ||
    !value.answers.every(
      (a) =>
        record(a) &&
        fields(a, ["id", "selected"], ["custom"]) &&
        typeof a.id === "string" &&
        Array.isArray(a.selected) &&
        a.selected.every((s) => typeof s === "string") &&
        (a.custom === undefined || typeof a.custom === "string"),
    )
  )
    throw new NativeFault("invalid_input");
  return value as QuestionAnswer;
}
function questionRequest(value: unknown): value is QuestionRequest {
  return (
    record(value) &&
    fields(value, ["questions"]) &&
    Array.isArray(value.questions) &&
    value.questions.every(
      (q) =>
        record(q) &&
        fields(
          q,
          ["id", "question"],
          ["detail", "header", "options", "multiSelect"],
        ) &&
        strings(q, ["id", "question"]) &&
        (q.detail === undefined || typeof q.detail === "string") &&
        (q.header === undefined || typeof q.header === "string") &&
        (q.multiSelect === undefined || typeof q.multiSelect === "boolean") &&
        (q.options === undefined ||
          (Array.isArray(q.options) &&
            q.options.every(
              (o) =>
                record(o) &&
                fields(o, ["label"], ["description"]) &&
                typeof o.label === "string" &&
                (o.description === undefined ||
                  typeof o.description === "string"),
            ))),
    )
  );
}
export function decodeRequest(raw: unknown): RequestFrame {
  check(
    record(raw) &&
      fields(raw, ["id", "operation", "value"]) &&
      Number.isSafeInteger(raw.id) &&
      Number(raw.id) > 0 &&
      operations.includes(raw.operation as Operation),
  );
  const v = raw.value;
  check(record(v));
  switch (raw.operation) {
    case "initialize": {
      const keys = [
        "nativeSessionId",
        "workingDirectory",
        "persistenceDirectory",
        "scope",
        "model",
        "apiKey",
        "apiUrl",
        "composition",
      ];
      check(
        fields(v, [...keys, "controlled", "restore"], ["previousRequestId"]) &&
          strings(v, keys) &&
          typeof v.controlled === "boolean" &&
          typeof v.restore === "boolean" &&
          (v.previousRequestId === undefined ||
            typeof v.previousRequestId === "string"),
      );
      break;
    }
    case "prompt":
      check(
        fields(v, ["requestId", "text"]) && strings(v, ["requestId", "text"]),
      );
      break;
    case "inspect":
      check(fields(v, ["requestId"]) && strings(v, ["requestId"]));
      break;
    case "answer":
      check(typeof v.callbackId === "string");
      if (v.unavailable === true)
        check(fields(v, ["callbackId", "unavailable"]));
      else {
        check(fields(v, ["callbackId", "answer"]));
        decodeAnswer(v.answer);
      }
      break;
    case "tool_result":
      check(
        fields(v, ["callbackId", "value"]) &&
          typeof v.callbackId === "string" &&
          record(v.value) &&
          fields(v.value, ["disposition", "text"]) &&
          ["returned", "rejected", "unavailable"].includes(
            String(v.value.disposition),
          ) &&
          typeof v.value.text === "string",
      );
      break;
    default:
      check(fields(v, []));
  }
  return raw as RequestFrame;
}
export function decodeResponse<K extends Operation>(
  operation: K,
  raw: unknown,
): OperationResponse[K] {
  check(record(raw));
  switch (operation) {
    case "initialize":
      check(
        fields(raw, [
          "composition",
          "nativeSessionId",
          "activation",
          "observationOnly",
          "previousTerminal",
        ]) &&
          strings(raw, ["composition", "nativeSessionId", "activation"]) &&
          typeof raw.observationOnly === "boolean" &&
          typeof raw.previousTerminal === "boolean",
      );
      break;
    case "prompt":
      check(
        fields(raw, ["status", "requestId", "activation", "observationOnly"]) &&
          raw.status === "accepted" &&
          raw.observationOnly === false &&
          strings(raw, ["requestId", "activation"]),
      );
      break;
    case "inspect":
      check(
        raw.status === "terminal"
          ? fields(raw, ["status", "outcome"]) &&
              outcomes.includes(raw.outcome as NativeOutcome)
          : fields(raw, ["status"]) &&
              ["unknown", "running"].includes(String(raw.status)),
      );
      break;
    default:
      check(fields(raw, []));
  }
  return raw as OperationResponse[K];
}
function decodeEvent(raw: unknown): NativeEvent {
  check(record(raw));
  const v = raw;
  const diagnostic = v.diagnostic;
  if (diagnostic !== undefined)
    check(
      record(diagnostic) &&
        fields(diagnostic, ["stage", "reason"]) &&
        [...operations, "configuration", "process", "profile"].includes(
          String(diagnostic.stage),
        ) &&
        diagnosticReasons.includes(
          diagnostic.reason as DeepSeekDiagnostic["reason"],
        ),
    );
  const base = ["type"],
    extra = ["diagnostic"];
  if (!["lost", "error"].includes(String(v.type)))
    check(typeof v.requestId === "string");
  switch (v.type) {
    case "lost":
      check(fields(v, base, extra));
      break;
    case "error":
      check(
        fields(v, base, [...extra, "requestId"]) &&
          (v.requestId === undefined || typeof v.requestId === "string"),
      );
      break;
    case "accepted":
      check(fields(v, [...base, "requestId"], extra));
      break;
    case "delta":
    case "text":
      check(
        fields(v, [...base, "requestId", "messageId", "text"], extra) &&
          strings(v, ["messageId", "text"]),
      );
      break;
    case "terminal":
      check(
        fields(v, [...base, "requestId", "outcome"], extra) &&
          outcomes.includes(v.outcome as NativeOutcome),
      );
      break;
    case "question":
      check(
        fields(v, [...base, "requestId", "callbackId", "request"], extra) &&
          typeof v.callbackId === "string" &&
          questionRequest(v.request),
      );
      break;
    case "question_unavailable":
      check(
        fields(v, [...base, "requestId", "callbackId"], extra) &&
          typeof v.callbackId === "string",
      );
      break;
    case "proposal":
      check(
        fields(v, [...base, "requestId", "callbackId", "proposal"], extra) &&
          typeof v.callbackId === "string" &&
          record(v.proposal) &&
          fields(v.proposal, ["name", "arguments"]) &&
          typeof v.proposal.name === "string" &&
          record(v.proposal.arguments),
      );
      break;
    default:
      throw new NativeFault("protocol_failure");
  }
  return raw as unknown as NativeEvent;
}
export type ResponseFrame =
  | { type: "event"; event: NativeEvent }
  | ({ type: "reply"; id: number } & (
      | { ok: true; value: unknown }
      | { ok: false; reason: DeepSeekDiagnostic["reason"] }
    ));
export function decodeFrame(raw: unknown): ResponseFrame {
  check(record(raw));
  if (raw.type === "event") {
    check(fields(raw, ["type", "event"]));
    return { type: "event", event: decodeEvent(raw.event) };
  }
  check(
    raw.type === "reply" && Number.isSafeInteger(raw.id) && Number(raw.id) > 0,
  );
  if (raw.ok === true) check(fields(raw, ["type", "id", "ok", "value"]));
  else
    check(
      raw.ok === false &&
        fields(raw, ["type", "id", "ok", "reason"]) &&
        diagnosticReasons.includes(raw.reason as DeepSeekDiagnostic["reason"]),
    );
  return raw as ResponseFrame;
}
