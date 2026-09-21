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
  | AttachReceipt
  | Connection
  | UserPreferences
  | TestUser
  | UserContext
  | HistoryPreview
  | ConnectionPage
  | ConnectionsRequest
  | SaveConnectionRequest
  | PreferencesRequest
  | SelectConnectionRequest
  | HistoryRequest
  | TestUserPage
  | NativeControlFrame
  | ExecutionOrigin
  | HostStatus
  | HostHealth;
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
      history?: HistoryPreview;
      targetCommandId?: never;
      generation?: never;
      nativeRunId?: never;
      interactionId?: never;
      answer?: never;
      surface?: never;
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
      text?: never;
      policy?: never;
      targetRunId?: never;
      history?: never;
      interactionId?: never;
      answer?: never;
      surface?: never;
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
      text?: never;
      policy?: never;
      targetRunId?: never;
      history?: never;
      targetCommandId?: never;
    };
/**
 * Closed command lifecycle; acceptance is immutable, local invalidation does not assert a model terminal.
 */
export type CommandRecord =
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "accepted";
      dispatch?: never;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "dispatching";
      /** Original attempt and append-once native correlation coordinates. */
      dispatch: DispatchAttempt;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "running";
      /** Original attempt and append-once native correlation coordinates. */
      dispatch: DispatchAttempt;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "terminal";
      /** Original attempt and append-once native correlation coordinates. */
      dispatch: DispatchAttempt;
      /** Explicitly observed model terminal outcome. */
      outcome: Outcome;
      failure?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "reconciliation_required";
      /** Original attempt and append-once native correlation coordinates. */
      dispatch: DispatchAttempt;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "invalidated";
      /** Local failure without asserting a model terminal. */
      failure: Failure;
      dispatch?: never;
      outcome?: never;
      cancelledBy?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "cancelled";
      /** Accepted local cancellation command identity. */
      cancelledBy: Id;
      dispatch?: never;
      outcome?: never;
      failure?: never;
      acknowledgement?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "acknowledged";
      /** Original attempt and append-once native correlation coordinates. */
      dispatch: DispatchAttempt;
      /** Closed control acknowledgement; never a model-turn outcome. */
      acknowledgement: Acknowledgement;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
      /**
       * Closed product record discriminator.
       */
      kind: "commandRecord";
      /** Immutable original command. */
      command: Command;
      /** Immutable original committed acceptance fact. */
      receipt: Receipt;
      /**
       * Closed command lifecycle projection.
       */
      state: "acknowledged";
      /**
       * Closed control acknowledgement; never a model-turn outcome.
       */
      /** Closed control acknowledgement; never a model-turn outcome. */
      acknowledgement: {
        /**
         * Closed variant discriminator.
         */
        type: "queued_cancelled";
        /** Queued prompt cancelled without a native dispatch. */
        targetCommandId: Id;
      };
      dispatch?: never;
      outcome?: never;
      failure?: never;
      cancelledBy?: never;
    };
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
  | "already_answered"
  | "storage_corrupt"
  | "connection_switch_pending"
  | "connection_required"
  | "authentication_required"
  | "context_unavailable"
  | "verification_cancelled"
  | "verification_refused";
/**
 * same_command preserves identity/content; reconcile_first checks the original operation; never forbids retry.
 */
export type Retry = "same_command" | "reconcile_first" | "never";
/**
 * Native control acknowledgement; never a model turn outcome.
 */
export type Acknowledgement =
  | {
      /**
       * Closed variant discriminator.
       */
      type: "cancel";
      /**
       * Provider confirms the control request, not a model terminal.
       */
      confirmation: "request_only" | "already_terminal";
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "respond";
      confirmation?: never;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "steer";
      confirmation?: never;
    };
/**
 * Closed stable events. Session events have no command, attempt observations name their exact attempt.
 */
export type Event =
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * text variant; all fields are data, never authentication or execution authority.
       */
      body: {
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
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Immutable command accepted atomically with its receipt; never a model terminal.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "command_accepted";
        /** Complete immutable original command, committed atomically with its receipt. */
        command: Command;
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * status variant; all fields are data, never authentication or execution authority.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "status";
        /**
         * Closed command lifecycle projection.
         */
        state: "dispatching" | "running" | "reconciliation_required";
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * terminal variant; all fields are data, never authentication or execution authority.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "terminal";
        /** Definite model-turn result; no implication about business side effects. */
        outcome: Outcome;
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * cancel_dispatched variant; all fields are data, never authentication or execution authority.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "cancel_dispatched";
        /**
         * Cancellation request transport confirmation only; does not manufacture a model terminal.
         */
        confirmation: "request_only" | "already_terminal" | "unsupported";
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * tool_proposal variant; all fields are data, never authentication or execution authority.
       */
      body: {
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
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * tool_result variant; all fields are data, never authentication or execution authority.
       */
      body: {
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
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Initial ordinary question publication; the matching Interaction is committed atomically.
       */
      body: {
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
        /** A live-generation callback. Restore preserves display history but always makes the previous callback unavailable. */
        callbackLifetime: CallbackLifetime;
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * First accepted response identity, committed atomically with the receipt and Interaction.
       */
      body: {
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
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Question lifecycle transition; cannot republish or replace its request.
       */
      body: {
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
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * error variant; all fields are data, never authentication or execution authority.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "error";
        /** Local failure without asserting a model terminal. */
        failure: Failure;
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable event data; never execution or authentication authority.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "invalidated";
        /** Local failure without asserting a model terminal. */
        failure: Failure;
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable event data; never execution or authentication authority.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "dispatch";
        /** Complete dispatch identity retained for replay and reconciliation. */
        attempt: DispatchAttempt;
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable event data; never execution or authentication authority.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "reconciled";
        /** Complete dispatch identity retained for replay and reconciliation. */
        attempt: DispatchAttempt;
        /**
         * Provider observation bound to this attempt and its current observer.
         */
        resolution:
          | "running"
          | "terminal"
          | "not_submitted"
          | "unknown"
          | "submitted"
          | "acknowledged";
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable product contract field.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "surface";
        /** Full surface recovery state committed with this event. */
        surface: SurfaceState;
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /**
       * Stable event data; never execution or authentication authority.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "session_rebound";
        /** Prior provider incarnation invalidated by this verified handoff. */
        previousGeneration: Id;
      };
      commandId?: never;
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /**
       * Stable event data; never execution or authentication authority.
       */
      body: {
        /**
         * Closed event discriminator.
         */
        type: "session_retired";
      };
      commandId?: never;
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable product contract field.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "acknowledged";
        /** Closed control acknowledgement; never a model-turn outcome. */
        acknowledgement: Acknowledgement;
      };
      /** Exact native dispatch attempt identity. */
      attemptId: Id;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable product contract field.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "cancelled";
        /** Accepted local cancellation command identity. */
        cancelledBy: Id;
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /**
       * Stable product contract field.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "session_recovery_unavailable";
      };
      commandId?: never;
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Stable product contract field.
       */
      body: {
        /**
         * Closed variant discriminator.
         */
        type: "acknowledged";
        /**
         * Closed control acknowledgement; never a model-turn outcome.
         */
        /** Closed control acknowledgement; never a model-turn outcome. */
        acknowledgement: {
          /**
           * Closed variant discriminator.
           */
          type: "queued_cancelled";
          /** Queued prompt cancelled without a native dispatch. */
          targetCommandId: Id;
        };
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Host-owned delivery fact; never provider authority or business success.
       */
      body: {
        type: "delivery_requested";
        operationId: Id;
        target: Id;
        proposal: {
          /** Provider tool name; not an approved execution action. */
          name: Id;
          arguments: {
            [k: string]: unknown;
          };
        };
      };
      attemptId?: never;
    }
  | {
      /**
       * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
       */
      schemaVersion: 5;
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
      /** Live provider incarnation token; rejects callbacks from previous incarnations. */
      generation: Id;
      /** Original command identity within the trusted namespace. */
      commandId: Id;
      /**
       * Host-owned delivery fact; never provider authority or business success.
       */
      body: {
        type: "delivery_recorded";
        operationId: Id;
        target: Id;
        /**
         * SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination.
         */
        contentHash: string;
        receiptRef: Id;
      };
      attemptId?: never;
    };
/**
 * A live-generation callback. Restore preserves display history but always makes the previous callback unavailable.
 */
export type CallbackLifetime = "generation_bound";
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
      generation?: never;
      commandId?: never;
      messageId?: never;
      text?: never;
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
      event?: never;
    }
  | {
      /**
       * Closed variant discriminator.
       */
      type: "resync_required";
      event?: never;
      generation?: never;
      commandId?: never;
      messageId?: never;
      text?: never;
    };
export type ConnectionSource =
  | {
      type: "custom_api";
      apiUrl: string;
      model: string;
      credentialType?: "api_key" | "auth_token";
      directory?: never;
    }
  | {
      type: "existing_config";
      directory?: string;
      model?: string;
      apiUrl?: never;
      credentialType?: never;
    };
export type PreferenceChange =
  | {
      set: Id;
      clear?: never;
    }
  | {
      clear: true;
      set?: never;
    };
/**
 * Private inherited Native-to-Host control frame. The descriptor is the trust boundary; this record only closes framing and payload shape.
 */
export type NativeControlFrame = NativeCall | NativeReply | NativeEvent;
export type NativeCall =
  | NativeCallAttach
  | NativeCallSuspend
  | NativeCallDetach
  | NativeCallSaveConnection
  | NativeCallMasterKey
  | NativeCallHealth;
export type NativeReply = NativeReplySuccess | NativeReplyFailure;

/**
 * Client command identity and complete canonical input; trusted namespace is supplied separately.
 */
export interface Command {
  /**
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
 * Explicit plain-text transcript preview bound to a target connection revision and frozen history watermark.
 */
export interface HistoryPreview {
  schemaVersion: 5;
  kind: "historyPreview";
  sessionId: Id;
  connectionId: Id;
  configRevision: Counter;
  throughSequence: Counter;
  /**
   * @maxItems 10000
   */
  commandIds: Id[];
  text: string;
  contentHash: Id;
  /**
   * @maxItems 10000
   */
  messageIds: Id[];
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
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
  /** Immutable provider phase selected by the host at command acceptance. */
  stageId: Id;
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
 * One active dispatch attempt. Origin identity is immutable; unknown native coordinates may be filled once. Only verified rebind changes observerGeneration.
 */
export interface DispatchAttempt {
  /** Provider-owned context session identifier; history alone cannot recreate it. */
  nativeSessionId: Id;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId?: Id;
  /** Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction. */
  nativeRequestId?: Id;
  /**
   * intent is stored before native submission; submitted has native acceptance; unknown requires reconciliation.
   */
  certainty: "intent" | "submitted" | "unknown";
  /** Stable identity of one dispatch attempt; never reused after positive non-submission proof. */
  attemptId: Id;
  /** Immutable provider incarnation that originated this attempt. */
  originGeneration: Id;
  /** Current verified provider incarnation permitted to observe this attempt. */
  observerGeneration: Id;
  /** Append-once native lookup key returned for ambiguous submission. */
  correlationId?: Id;
  /** Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads. */
  nativeThreadId?: Id;
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
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
   * Deleted is an upstream deletion; invalidated is a product-side loss of action authority. Neither may reactivate.
   */
  status: "active" | "deleted" | "invalidated";
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
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
  /**
   * Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.
   */
  status: "active" | "recovery_required" | "retired";
  selectedConnectionId?: Id;
  currentStageId?: Id;
  /**
   * @maxItems 128
   */
  stages: ContextStage[];
  /**
   * Explicit next-prompt intent. The current phase remains available for old receipts and device deliveries until the new phase is admitted.
   */
  freshContext?: true;
}
/**
 * One provider context phase. Native generations may change only through verified recovery; binding is owned here.
 */
export interface ContextStage {
  stageId: Id;
  connectionId: Id;
  configRevision: Counter;
  binding: Binding;
  capabilities: Capabilities;
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
  /** Provider-owned context session identifier; history alone cannot recreate it. */
  nativeSessionId: Id;
  /** Exact immutable configuration identity and revision. */
  config: ConfigRef;
  /** Provider-owned model-turn/run identifier, required when the provider exposes it. */
  nativeRunId?: Id;
  /** Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction. */
  nativeRequestId?: Id;
  /** SHA-256 identity of the normalized absolute workspace path. Filesystem containment remains owned by the provider adapter and composition root. */
  workspaceId: Id;
  /** Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads. */
  nativeThreadId?: Id;
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
}
/**
 * Single-use provider callback with immutable command/native correlation, expiry and lifetime.
 */
export interface Interaction {
  /**
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
  /** A live-generation callback. Restore preserves display history but always makes the previous callback unavailable. */
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
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
  status:
    | "pending"
    | "delivered"
    | "receipt_recorded"
    | "reconciliation_required";
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
   * Exact product wire version; Versions 1–4 are rejected without migration or fallback.
   */
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
  schemaVersion: 5;
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
 * A user-owned named provider connection with immutable configuration and credential revisions; contains opaque references, never secrets.
 */
export interface Connection {
  schemaVersion: 5;
  kind: "connection";
  connectionId: Id;
  name: string;
  provider: "codex" | "claude" | "deepseek";
  configRevision: Counter;
  profile: "conversation" | "controlled_tools";
  status:
    | "unverified"
    | "ready"
    | "authentication_required"
    | "invalid"
    | "deleted";
  source: ConnectionSource;
}
/**
 * Independent optional selections owned by the current test user.
 */
export interface UserPreferences {
  schemaVersion: 5;
  kind: "userPreferences";
  defaultConnectionId?: Id;
  selectedSessionId?: Id;
}
/**
 * TestUser product wire record; validated against the V5 schema.
 */
export interface TestUser {
  schemaVersion: 5;
  kind: "testUser";
  userId: Id;
  displayName: string;
  nameKey: string;
}
/**
 * UserContext product wire record; validated against the V5 schema.
 */
export interface UserContext {
  schemaVersion: 5;
  kind: "userContext";
  user: TestUser;
  generation: Id;
}
export interface ConnectionPage {
  schemaVersion: 5;
  kind: "connectionPage";
  /**
   * @maxItems 128
   */
  connections: Connection[];
  preferences: UserPreferences;
}
export interface ConnectionsRequest {
  schemaVersion: 5;
  kind: "connectionsRequest";
}
export interface SaveConnectionRequest {
  schemaVersion: 5;
  kind: "saveConnectionRequest";
  connection: Connection;
  expectedRevision: Counter | null;
}
export interface PreferencesRequest {
  schemaVersion: 5;
  kind: "preferencesRequest";
  patch: PreferencesPatch;
}
/**
 * Missing fields remain unchanged; set replaces and clear removes one preference atomically.
 */
export interface PreferencesPatch {
  defaultConnectionId?: PreferenceChange;
  selectedSessionId?: PreferenceChange;
}
export interface SelectConnectionRequest {
  schemaVersion: 5;
  kind: "selectConnectionRequest";
  sessionId: Id;
  connectionId: Id;
  freshContext: boolean;
}
export interface HistoryRequest {
  schemaVersion: 5;
  kind: "historyRequest";
  sessionId: Id;
  connectionId: Id;
  recent?: number;
}
export interface TestUserPage {
  schemaVersion: 5;
  kind: "testUserPage";
  /**
   * @maxItems 128
   */
  users: TestUser[];
  current?: UserContext;
}
export interface NativeCallAttach {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "attach";
  data: NativeAttachData;
}
export interface NativeAttachData {
  channel: Id;
  context: UserContext;
}
export interface NativeCallSuspend {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "suspend";
  data: NativeSuspendData;
}
export interface NativeSuspendData {
  context: UserContext;
}
export interface NativeCallDetach {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "detach";
  data: NativeDetachData;
}
export interface NativeDetachData {
  channel: Id;
}
export interface NativeCallSaveConnection {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "saveConnection";
  data: NativeSaveConnectionData;
}
export interface NativeSaveConnectionData {
  generation: Id;
  connection: Connection;
  expected: Counter | null;
  secret: string | null;
}
export interface NativeCallMasterKey {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "masterKey";
  data: NativeMasterKeyData;
}
export interface NativeMasterKeyData {
  create: boolean;
}
export interface NativeCallHealth {
  schemaVersion: 5;
  kind: "nativeCall";
  id: Counter;
  method: "health";
  data: {};
}
export interface NativeReplySuccess {
  schemaVersion: 5;
  kind: "nativeReply";
  id: Counter;
  ok: true;
  value: unknown;
}
export interface NativeReplyFailure {
  schemaVersion: 5;
  kind: "nativeReply";
  id: Counter;
  ok: false;
}
export interface NativeEvent {
  schemaVersion: 5;
  kind: "nativeEvent";
  channel: Id;
  message: unknown;
}
/**
 * Non-secret AI operation provenance carried only on the desktop-owned execution pipe.
 */
export interface ExecutionOrigin {
  schemaVersion: 5;
  kind: "executionOrigin";
  namespace: Namespace;
  /** Native-selected user generation; checked against the current trusted registry before every tool call. */
  userGeneration: Id;
  operationId: Id;
  provider: "codex" | "claude" | "deepseek";
  config: ConfigRef;
}
export interface HostStatus {
  schemaVersion: 5;
  kind: "hostStatus";
  generation: Counter;
  phase: "stopped" | "starting" | "ready" | "stopping" | "failed";
  source: "development_override" | "bundled_resource";
  version: string;
  diagnostic?: HostDiagnostic;
  /**
   * @maxItems 64
   */
  recent: HostDiagnostic[];
}
export interface HostDiagnostic {
  stage:
    | "runtime_package"
    | "host_process"
    | "configuration"
    | "authentication"
    | "storage"
    | "shutdown";
  code:
    | "runtime_missing"
    | "runtime_invalid"
    | "unsupported_version"
    | "host_start_failed"
    | "host_exited"
    | "readiness_timeout"
    | "configuration_invalid"
    | "authentication_required"
    | "storage_corrupt"
    | "cleanup_incomplete"
    | "control_closed";
  action:
    | "prepare_runtime"
    | "reinstall_runtime"
    | "restart_host"
    | "check_configuration"
    | "check_credentials"
    | "check_storage";
  atMs: Counter;
}
export interface HostHealth {
  schemaVersion: 5;
  kind: "hostHealth";
  ready: true;
  protocol: 1;
}

/**
 * Selected product ACP extensions; capability metadata is never execution authority.
 */
export interface Negotiation {
  /**
   * Exact product contract version.
   */
  contractVersion: 5;
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
