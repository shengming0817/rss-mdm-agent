// @generated from schema/runtime.schema.json. Do not edit.

/**
 * Product reliability records only. No record authenticates a caller, grants approval or proves business execution. Standard ACP/A2UI schemas retain their upstream owners.
 */
export type WireRecord =
  | Command
  | Receipt
  | CommandRecord
  | Event
  | Session
  | Interaction
  | Delivery
  | SurfaceBinding
  | SurfaceAction;
export type Id = string;
export type Counter = number;
export type Input =
  | {
      type: "prompt";
      text: string;
      policy: "queue_next" | "steer";
      targetRunId?: Id;
    }
  | {
      type: "cancel";
      targetCommandId: Id;
      generation: Id;
      nativeRunId?: Id;
    }
  | {
      type: "respond";
      interactionId: Id;
      generation: Id;
      nativeRunId?: Id;
      answer: {
        [k: string]: unknown;
      };
    };
export type CommandState =
  | "accepted"
  | "dispatching"
  | "running"
  | "terminal"
  | "reconciliation_required";
export type Outcome =
  | "completed"
  | "interrupted"
  | "refused"
  | "limit_reached"
  | "failed";
export type ErrorCode =
  | "invalid_input"
  | "unsupported_version"
  | "unsupported_capability"
  | "permission_denied"
  | "content_conflict"
  | "revision_conflict"
  | "stale_binding"
  | "expired"
  | "unavailable"
  | "reconciliation_required"
  | "limit_exceeded"
  | "cursor_expired"
  | "session_gone"
  | "already_answered";
export type Retry = "same_command" | "reconcile_first" | "never";
export type EventBody =
  | {
      type: "text";
      messageId: Id;
      text: string;
    }
  | {
      type: "status";
      state: CommandState;
    }
  | {
      type: "terminal";
      outcome: Outcome;
    }
  | {
      type: "cancel_dispatched";
      confirmation: "request_only" | "already_terminal" | "unsupported";
    }
  | {
      type: "tool_proposal";
      proposalId: Id;
      name: Id;
      arguments: {
        [k: string]: unknown;
      };
    }
  | {
      type: "tool_result";
      proposalId: Id;
      disposition: "returned" | "rejected" | "unavailable";
      text: string;
    }
  | {
      type: "interaction";
      interactionId: Id;
      status: "pending" | "answered" | "expired" | "unavailable";
    }
  | {
      type: "error";
      failure: Failure;
    };
export type CapabilityState = "supported" | "unsupported" | "unknown";

export interface Command {
  schemaVersion: 2;
  kind: "command";
  sessionId: Id;
  commandId: Id;
  expiresAtMs: Counter;
  input: Input;
}
export interface Receipt {
  schemaVersion: 2;
  kind: "receipt";
  namespace: Namespace;
  commandId: Id;
  contentHash: string;
  acceptedAtMs: Counter;
  retryUntilMs: Counter;
  receiptUntilMs: Counter;
  acceptedRevision: Counter;
}
export interface Namespace {
  tenantId: Id;
  principalId: Id;
  authorityId: Id;
  sessionId: Id;
}
export interface CommandRecord {
  schemaVersion: 2;
  kind: "commandRecord";
  command: Command;
  receipt: Receipt;
  state: CommandState;
  dispatch?: Dispatch;
  outcome?: Outcome;
  failure?: Failure;
}
export interface Dispatch {
  generation: Id;
  nativeSessionId: Id;
  nativeRunId?: Id;
  nativeRequestId?: Id;
  certainty: "not_sent" | "submitted" | "unknown";
}
export interface Failure {
  code: ErrorCode;
  retry: Retry;
}
export interface Event {
  schemaVersion: 2;
  kind: "event";
  namespace: Namespace;
  eventId: Id;
  sequence: Counter;
  commandId: Id;
  generation: Id;
  body: EventBody;
}
export interface Session {
  schemaVersion: 2;
  kind: "session";
  namespace: Namespace;
  revision: Counter;
  lastSequence: Counter;
  binding: Binding;
  capabilities: Capabilities;
  status: "active" | "retired";
}
export interface Binding {
  provider: Id;
  providerVersion: Id;
  adapterVersion: Id;
  generation: Id;
  accountRef: Id;
  nativeSessionId: Id;
  config: ConfigRef;
  nativeRunId?: Id;
  nativeRequestId?: Id;
}
export interface ConfigRef {
  id: Id;
  revision: Id;
}
export interface Capabilities {
  continuation: "same_process" | "across_processes" | "unsupported" | "unknown";
  cancellation:
    | "request_only"
    | "terminal_acknowledged"
    | "unsupported"
    | "unknown";
  tools: "host_mediated" | "provider_managed" | "disabled" | "unknown";
  steer: CapabilityState;
  fork: CapabilityState;
  subagent: CapabilityState;
  terminal: CapabilityState;
  structuredQuestion: CapabilityState;
  multimodal: CapabilityState;
}
export interface Interaction {
  schemaVersion: 2;
  kind: "interaction";
  namespace: Namespace;
  interactionId: Id;
  commandId: Id;
  generation: Id;
  nativeRunId?: Id;
  nativeRequestId: Id;
  expiresAtMs: Counter;
  status: "pending" | "answered" | "expired" | "unavailable";
  responseCommandId?: Id;
  callbackLifetime: "generation_bound" | "provider_resumable";
}
export interface Delivery {
  schemaVersion: 2;
  kind: "delivery";
  namespace: Namespace;
  operationId: Id;
  eventId: Id;
  target: Id;
  contentHash: string;
  retry: "receiver_idempotent" | "reconcile_first" | "never";
  status: "pending" | "delivered" | "reconciliation_required";
  attempts: Counter;
  nextAttemptAtMs: Counter;
}
export interface SurfaceBinding {
  schemaVersion: 2;
  kind: "surface";
  namespace: Namespace;
  generation: Id;
  nativeRunId: Id;
  surfaceId: Id;
  surfaceInstanceId: Id;
  revision: Counter;
  interactionId: Id;
  sourceComponentId: Id;
  eventName: Id;
  catalogId: Id;
  catalogVersion: Id;
  a2uiVersion: "v0.9.1";
}
export interface SurfaceAction {
  schemaVersion: 2;
  kind: "surfaceAction";
  sessionId: Id;
  commandId: Id;
  surfaceInstanceId: Id;
  surfaceRevision: Counter;
  interactionId: Id;
  generation: Id;
  nativeRunId: Id;
}
