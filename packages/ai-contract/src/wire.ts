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
  | SurfaceState
  | SurfaceAction
  | SnapshotPage
  | SessionPage
  | SnapshotRequest
  | ListRequest
  | AttachRequest
  | DetachRequest
  | ResumeRequest
  | ActionRequest
  | AccessUpdate
  | AttachReceipt;
/**
 * Opaque ASCII correlation identifier (1–128 characters); never an authentication credential.
 */
export type Id = string;
/**
 * Nonnegative integer in the shared JavaScript safe-integer range.
 */
export type Counter = number;
/**
 * Product command inputs; provider-specific formats remain adapter-owned.
 */
export type Input =
  | {
      /**
       * Closed variant discriminator.
       */
      type: "prompt";
      /**
       * Untrusted model/user text subject to the whole-envelope budgets.
       */
      text: string;
      /**
       * queue_next serializes later work; steer must match the currently active native run.
       */
      policy: "queue_next" | "steer";
      /** Exact active native run required for steer; forbidden for queue_next. */
      targetRunId?: Id;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "cancel";
      /** Original accepted command being cancelled. */
      targetCommandId: Id;
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
      nativeRunId?: Id;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "respond";
      /** Single-use interaction identity within the namespace. */
      interactionId: Id;
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
      nativeRunId?: Id;
      /**
       * Untrusted JSON response data; cannot carry approval authority.
       */
      answer: {
        [k: string]: unknown;
      };
      /** Mandatory for an interaction associated with a surface; cannot bypass deleted/stale state. */
      surface?: SurfaceReference;
    };
/**
 * accepted persists intent; dispatching persists dispatch intent; running has native confirmation; terminal has a definite outcome; reconciliation_required forbids blind resubmission.
 */
export type CommandState =
  | "accepted"
  | "dispatching"
  | "running"
  | "terminal"
  | "reconciliation_required";
/**
 * Definite model-turn outcome; does not establish process exit or business-side-effect completion.
 */
export type Outcome =
  | "completed"
  | "cancelled"
  | "refused"
  | "max_tokens"
  | "max_turn_requests"
  | "failed";
/**
 * Closed value-free error category; diagnostics never include model text or credentials.
 */
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
/**
 * same_command preserves identity/content; reconcile_first checks the original operation; never forbids retry.
 */
export type Retry = "same_command" | "reconcile_first" | "never";
/**
 * Stable product observation. Tool proposals and results are untrusted and cannot issue execution authority.
 */
export type EventBody =
  | {
      /**
       * Closed variant discriminator.
       */
      type: "text";
      /** Stable product message correlation identifier. */
      messageId: Id;
      /**
       * Untrusted model/user text subject to the whole-envelope budgets.
       */
      text: string;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "status";
      /** Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence. */
      state: CommandState;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "terminal";
      /** Definite model-turn result; no implication about business side effects. */
      outcome: Outcome;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "cancel_dispatched";
      /**
       * Cancellation request transport confirmation only; does not manufacture a model terminal.
       */
      confirmation: "request_only" | "already_terminal" | "unsupported";
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "tool_proposal";
      /** Untrusted tool proposal correlation identifier. */
      proposalId: Id;
      /** Provider tool name; not an approved execution action. */
      name: Id;
      /**
       * Untrusted tool JSON arguments, including keys, count toward product budgets.
       */
      arguments: {
        [k: string]: unknown;
      };
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "tool_result";
      /** Untrusted tool proposal correlation identifier. */
      proposalId: Id;
      /**
       * Protocol tool-result disposition, not authoritative business execution status.
       */
      disposition: "returned" | "rejected" | "unavailable";
      /**
       * Untrusted model/user text subject to the whole-envelope budgets.
       */
      text: string;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "interaction";
      /** Single-use interaction identity within the namespace. */
      interactionId: Id;
      /**
       * First publication of an ordinary user question.
       */
      status: "pending";
      /** Required for the first pending event and equal to the newly committed Interaction request; forbidden on later lifecycle events. */
      request: InteractionRequest;
      /** Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected. */
      expiresAtMs: Counter;
      /** generation_bound cannot survive callback loss; provider_resumable requires verified native restoration. */
      callbackLifetime: CallbackLifetime;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "interaction";
      /** Single-use interaction identity within the namespace. */
      interactionId: Id;
      /**
       * The first accepted response consumed this interaction.
       */
      status: "answered";
      /** Accepted response command which atomically consumed the interaction; present only when answered. */
      responseCommandId: Id;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "interaction";
      /** Single-use interaction identity within the namespace. */
      interactionId: Id;
      /**
       * Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.
       */
      status: "expired" | "unavailable";
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "error";
      /** Closed failure category and retry discipline. */
      failure: Failure;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "surface";
      /** Full surface recovery state committed with this event. */
      surface: SurfaceState;
    };
/**
 * generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.
 */
export type CallbackLifetime = "generation_bound" | "provider_resumable";
/**
 * Only supported enables an operation; unknown and unsupported fail closed.
 */
export type CapabilityState = "supported" | "unsupported" | "unknown";
export type Subscription =
  | {
      /**
       * Closed variant discriminator.
       */
      type: "event";
      /** Stable Host event. */
      event: Event;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "delta";
      /** Exact native provider incarnation. */
      generation: Id;
      /** Product command identity. */
      commandId: Id;
      /** Identity of the streamed message within a command. */
      messageId: Id;
      /**
       * Untrusted display text.
       */
      text: string;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "resync_required";
    };

/**
 * Client command identity and complete canonical input; trusted namespace is supplied separately.
 */
export interface Command {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "command";
  /** Logical session identifier, never reusable after retirement. */
  sessionId: Id;
  /** Client-generated idempotency key; reuse only with identical canonical content. */
  commandId: Id;
  /** Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected. */
  expiresAtMs: Counter;
  /** Complete command content included in its canonical fingerprint. */
  input: Input;
}
/**
 * Surface revision checked atomically when accepting an action response.
 */
export interface SurfaceReference {
  /** Exact product surface instance associated with this response. */
  instanceId: Id;
  /** Current surface revision checked atomically during response acceptance. */
  revision: Counter;
}
/**
 * Immutable acceptance fact. Only an actual committed store makes it durable; it is not a model terminal.
 */
export interface Receipt {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "receipt";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Client-generated idempotency key; reuse only with identical canonical content. */
  commandId: Id;
  /**
   * SHA-256 of JCS(command), including every command field; namespace is a separate storage key.
   */
  contentHash: string;
  /** UTC epoch milliseconds at committed acceptance. */
  acceptedAtMs: Counter;
  /** Inclusive same-command retry deadline, no later than command expiry. */
  retryUntilMs: Counter;
  /** Inclusive stored-receipt deadline; cannot be shorter than the retry deadline. */
  receiptUntilMs: Counter;
  /** Session revision which atomically accepted this command. */
  acceptedRevision: Counter;
}
/**
 * Trusted tenant/principal/authority/logical-session storage scope supplied by authenticated ingress.
 */
export interface Namespace {
  /** Authenticated tenant scope. */
  tenantId: Id;
  /** Authenticated principal scope. */
  principalId: Id;
  /** Authenticated authority scope. */
  authorityId: Id;
  /** Logical session identifier, never reusable after retirement. */
  sessionId: Id;
}
/**
 * Single inbox/dispatch ledger; no second provider queue owns the same command.
 */
export interface CommandRecord {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "commandRecord";
  /** Immutable original command. */
  command: Command;
  /** Immutable original committed acceptance fact. */
  receipt: Receipt;
  /** Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence. */
  state: CommandState;
  /** Native correlation persisted before/with dispatch; never reconstructed from UI history. */
  dispatch?: Dispatch;
  /** Definite model-turn result; no implication about business side effects. */
  outcome?: Outcome;
  /** Closed failure category and retry discipline. */
  failure?: Failure;
}
/**
 * Persisted native correlation and certainty; unknown requires reconciliation before any further send.
 */
export interface Dispatch {
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Provider-owned context session identifier; history alone cannot recreate it. */
  nativeSessionId: Id;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId?: Id;
  /** Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction. */
  nativeRequestId?: Id;
  /**
   * submitted has native acceptance; unknown requires reconciliation; not_sent has evidence no submission occurred.
   */
  certainty: "not_sent" | "submitted" | "unknown";
}
/**
 * Value-free failure and explicit retry discipline.
 */
export interface Failure {
  /** Closed diagnostic category without input values. */
  code: ErrorCode;
  /** Explicit retry discipline; uncertainty never authorizes blind resubmission. */
  retry: Retry;
}
/**
 * Committed stable event in one namespace; token deltas are excluded from durable ordering.
 */
export interface Event {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "event";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Stable unique event identifier within the namespace. */
  eventId: Id;
  /** Strictly increasing stable-event counter; attach cursors are exclusive. */
  sequence: Counter;
  /** Client-generated idempotency key; reuse only with identical canonical content. */
  commandId: Id;
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Stable observation committed before publication. */
  body: EventBody;
}
/**
 * Immutable untrusted provider question payload. Subject to whole-record JSON budgets; never authentication, permission or execution approval.
 */
export interface InteractionRequest {
  [k: string]: unknown;
}
/**
 * Single surface record: association, lifecycle and bounded upstream recovery content.
 */
export interface SurfaceState {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "surface";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId: Id;
  /** Upstream A2UI surface identifier. */
  surfaceId: Id;
  /** Fresh product identity for each surface creation; deletion permanently invalidates old actions. */
  surfaceInstanceId: Id;
  /** Monotonic CAS revision of this product record. */
  revision: Counter;
  /** Single-use interaction identity within the namespace. */
  interactionId: Id;
  /** Exact upstream source component allowed to emit this action. */
  sourceComponentId: Id;
  /** Exact upstream action name associated with this interaction. */
  eventName: Id;
  /** Negotiated upstream catalog identity. */
  catalogId: Id;
  /** Negotiated fixed catalog version; not a renderer implementation claim. */
  catalogVersion: Id;
  /**
   * Exact negotiated upstream A2UI version.
   */
  a2uiVersion: "v0.9.1";
  /**
   * Persisted lifecycle; deleted is a permanent tombstone for this instance.
   */
  status: "active" | "deleted";
  /**
   * Bounded unchanged upstream messages needed to rebuild this instance; not a second A2UI schema.
   *
   * @maxItems 128
   */
  messages: {
    [k: string]: unknown;
  }[];
}
/**
 * Logical session state and stable event watermark committed at one revision.
 */
export interface Session {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "session";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Monotonic CAS revision of this product record. */
  revision: Counter;
  /** Highest committed stable-event sequence at this session revision. */
  lastSequence: Counter;
  /** Exact provider incarnation and native context identity. */
  binding: Binding;
  /** Capabilities bound to this exact provider/configuration/account incarnation. */
  capabilities: Capabilities;
  /**
   * Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.
   */
  status: "active" | "retired";
}
/**
 * Provider context identity. Version, configuration, account and generation bind every capability and callback.
 */
export interface Binding {
  /** Provider adapter identity. */
  provider: Id;
  /** Pinned native provider implementation version. */
  providerVersion: Id;
  /** Adapter implementation version used for capability verification. */
  adapterVersion: Id;
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Opaque account reference; no token, key or account-directory contents. */
  accountRef: Id;
  /** Provider-owned context session identifier; history alone cannot recreate it. */
  nativeSessionId: Id;
  /** Exact immutable configuration identity and revision. */
  config: ConfigRef;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId?: Id;
  /** Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction. */
  nativeRequestId?: Id;
}
/**
 * Immutable configuration identity and revision; contains no credentials.
 */
export interface ConfigRef {
  /** Configuration identifier; resolve credentials outside the wire. */
  id: Id;
  /** Immutable configuration revision; changing it invalidates prior capability evidence. */
  revision: Id;
}
/**
 * Capabilities established for one exact provider binding, never execution authorization.
 */
export interface Capabilities {
  /**
   * same_process resumes only a live context; across_processes requires verified provider restoration; unsupported/unknown cannot resume.
   */
  continuation: "same_process" | "across_processes" | "unsupported" | "unknown";
  /**
   * request_only confirms sending; terminal_acknowledged requires native terminal evidence; unsupported/unknown cannot promise cancellation.
   */
  cancellation:
    | "request_only"
    | "terminal_acknowledged"
    | "unsupported"
    | "unknown";
  /**
   * host_mediated still requires containment evidence; provider_managed is not controlled execution; disabled/unknown cannot enable tools.
   */
  tools: "host_mediated" | "provider_managed" | "disabled" | "unknown";
  /** Whether an active native run accepts targeted steering. */
  steer: CapabilityState;
  /** Whether the provider supports an explicit context fork. */
  fork: CapabilityState;
  /** Whether the provider supports child agents; not execution authorization. */
  subagent: CapabilityState;
  /** Whether the provider exposes a terminal facility; not the command terminal state. */
  terminal: CapabilityState;
  /** Whether a native structured callback can be represented and answered. */
  structuredQuestion: CapabilityState;
  /** Whether provider-specific multimodal input is available through an adapter extension. */
  multimodal: CapabilityState;
  /** Whether additional prompts may be queued while a run is active. */
  queue: CapabilityState;
}
/**
 * Single-use provider callback with immutable command/native correlation, expiry and lifetime.
 */
export interface Interaction {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "interaction";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Single-use interaction identity within the namespace. */
  interactionId: Id;
  /** Client-generated idempotency key; reuse only with identical canonical content. */
  commandId: Id;
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId?: Id;
  /** Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected. */
  expiresAtMs: Counter;
  /**
   * Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.
   */
  status: "pending" | "answered" | "expired" | "unavailable";
  /** Accepted response command which atomically consumed the interaction; present only when answered. */
  responseCommandId?: Id;
  /** generation_bound cannot survive callback loss; provider_resumable requires verified native restoration. */
  callbackLifetime: CallbackLifetime;
  /** Provider-owned callback identifier, immutable and unique within a session generation; distinct from the parent dispatch request. */
  nativeCallbackId: Id;
  /** Immutable untrusted question payload retained for display; does not restore a lost native callback or grant approval. */
  request: InteractionRequest;
  /**
   * Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle.
   */
  category: "question";
}
/**
 * Reliable cross-service outbox record bound to an immutable event and target.
 */
export interface Delivery {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "delivery";
  /** Trusted storage isolation scope; not copied from model or action content. */
  namespace: Namespace;
  /** Stable receiver idempotency key for this delivery; cannot be rebound to another event/target. */
  operationId: Id;
  /** Stable unique event identifier within the namespace. */
  eventId: Id;
  /** Opaque reliable-delivery destination identifier. */
  target: Id;
  /**
   * SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination.
   */
  contentHash: string;
  /**
   * Explicit retry discipline; uncertainty never authorizes blind resubmission.
   */
  retry: "receiver_idempotent" | "reconcile_first" | "never";
  /**
   * Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.
   */
  status: "pending" | "delivered" | "reconciliation_required";
  /** Monotonic count of delivery attempts. */
  attempts: Counter;
  /** UTC epoch-millisecond earliest eligible retry time. */
  nextAttemptAtMs: Counter;
}
/**
 * Product metadata accompanying an unchanged upstream action; association does not grant permission.
 */
export interface SurfaceAction {
  /**
   * Exact product wire version; V1 is rejected without migration or fallback.
   */
  schemaVersion: 2;
  /**
   * Closed product record discriminator.
   */
  kind: "surfaceAction";
  /** Logical session identifier, never reusable after retirement. */
  sessionId: Id;
  /** Client-generated idempotency key; reuse only with identical canonical content. */
  commandId: Id;
  /** Fresh product identity for each surface creation; deletion permanently invalidates old actions. */
  surfaceInstanceId: Id;
  /** Exact current surface revision required to accept this action. */
  surfaceRevision: Counter;
  /** Single-use interaction identity within the namespace. */
  interactionId: Id;
  /** Live provider incarnation token; rejects callbacks from previous incarnations. */
  generation: Id;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId: Id;
}
export interface SnapshotPage {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "snapshotPage";
  /** Identity shared by all pages from one immutable read view. */
  snapshotId: Id;
  /** Zero-based page order within this snapshot. */
  pageIndex: Counter;
  /** Session at the snapshot watermark. */
  session: Session;
  /** Stable event watermark shared by every page. */
  cursor: Counter;
  /**
   * Stable events at or below the watermark.
   */
  events: Event[];
  /**
   * Command projections at the watermark.
   */
  commands: CommandRecord[];
  /**
   * Interaction display state; does not restore a native callback.
   */
  interactions: Interaction[];
  /**
   * Bounded original A2UI recovery messages and their associations.
   */
  surfaces: SurfaceState[];
  /** Opaque continuation; absent at end of the read view. */
  next?: Id;
}
export interface SessionPage {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "sessionPage";
  /**
   * Caller-scoped sessions in this immutable page.
   */
  items: Session[];
  /** Opaque continuation; absent at end of the read view. */
  next?: Id;
}
export interface SnapshotRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "snapshotRequest";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
  /** Bounded page query with an opaque caller-bound continuation. */
  query: PageQuery;
}
export interface PageQuery {
  /**
   * Maximum records in this page, from 1 to 256.
   */
  limit: number;
  /** Opaque continuation of one immutable read view; expires independently of the session. */
  continuation?: Id;
}
export interface ListRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "listRequest";
  /** Bounded page query with an opaque caller-bound continuation. */
  query: PageQuery;
}
export interface AttachRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "attachRequest";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
  /** Last stable sequence consumed before attaching. */
  after: Counter;
  /** Connection-local attachment identity, echoed on every update. */
  attachmentId: Id;
}
export interface DetachRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "detachRequest";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
  /** Connection-local attachment identity, echoed on every update. */
  attachmentId: Id;
}
export interface ResumeRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "resumeRequest";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
}
export interface ActionRequest {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "actionRequest";
  /** Product action association, checked independently of untrusted upstream context. */
  metadata: SurfaceAction;
  /**
   * Unchanged upstream A2UI client message, validated against the negotiated schema.
   */
  message: {
    [k: string]: unknown;
  };
  /** Command expiry in Unix milliseconds. */
  expiresAtMs: Counter;
}
export interface AccessUpdate {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "accessUpdate";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
  /** Connection-local attachment identity, echoed on every update. */
  attachmentId: Id;
  /** Stable event, ephemeral delta, or explicit resynchronization signal. */
  update: Subscription;
}
export interface AttachReceipt {
  /**
   * Exact product contract version; no legacy readers.
   */
  schemaVersion: 2;
  /**
   * Closed record discriminator.
   */
  kind: "attachReceipt";
  /** Product session identity within the authenticated caller namespace. */
  sessionId: Id;
  /** Connection-local attachment identity, echoed on every update. */
  attachmentId: Id;
  /** Last stable sequence consumed before attaching. */
  after: Counter;
}

/**
 * Selected product ACP extensions; capability metadata is never execution authority.
 */
export interface Negotiation {
  /**
   * Exact product contract version.
   */
  contractVersion: 2;
  /**
   * Exact ACP protocol version.
   */
  acp: 1;
  /**
   * Host supports transactional receipt semantics; memory doubles simulate this only.
   */
  durableReceipts: boolean;
  /**
   * Host supports stable snapshot-to-event attachment.
   */
  cursorAttach: boolean;
  /** Explicitly selected upstream version and product catalog. */
  a2ui?: A2UiNegotiation;
}

/**
 * Explicitly selected upstream version and product catalog.
 */
export interface A2UiNegotiation {
  /**
   * Fixed upstream protocol version.
   */
  version: "v0.9.1";
  /**
   * Fixed product catalog identity.
   */
  catalogId: "urn:rss-mdm-agent:a2ui:interaction";
  /**
   * Fixed product catalog revision.
   */
  catalogVersion: "1";
}
