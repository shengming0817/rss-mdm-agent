// @ts-nocheck
// @generated from canonical schemas; do not edit.
const helper0 = function ucs2length(str) {
  const len = str.length;
  let length = 0;
  let pos = 0;
  let value;
  while (pos < len) {
    length++;
    value = str.charCodeAt(pos++);
    if (value >= 0xd800 && value <= 0xdbff && pos < len) {
      // high surrogate, and there is a next character
      value = str.charCodeAt(pos);
      if ((value & 0xfc00) === 0xdc00) pos++; // low surrogate
    }
  }
  return length;
};
("use strict");
export const validate = validate20;
export default validate20;
const schema31 = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "urn:rss-mdm-agent:ai-runtime:2",
  title: "WireRecord",
  description:
    "Product reliability records only. No record authenticates a caller, grants approval or proves business execution. Standard ACP/A2UI schemas retain their upstream owners.",
  oneOf: [
    { $ref: "#/$defs/Command" },
    { $ref: "#/$defs/Receipt" },
    { $ref: "#/$defs/CommandRecord" },
    { $ref: "#/$defs/Event" },
    { $ref: "#/$defs/Session" },
    { $ref: "#/$defs/Interaction" },
    { $ref: "#/$defs/Delivery" },
    { $ref: "#/$defs/SurfaceState" },
    { $ref: "#/$defs/SurfaceAction" },
    { $ref: "#/$defs/SnapshotPage" },
    { $ref: "#/$defs/SessionPage" },
    { $ref: "#/$defs/SnapshotRequest" },
    { $ref: "#/$defs/ListRequest" },
    { $ref: "#/$defs/AttachRequest" },
    { $ref: "#/$defs/DetachRequest" },
    { $ref: "#/$defs/ResumeRequest" },
    { $ref: "#/$defs/ActionRequest" },
    { $ref: "#/$defs/AccessUpdate" },
    { $ref: "#/$defs/AttachReceipt" },
  ],
  $defs: {
    Id: {
      type: "string",
      minLength: 1,
      maxLength: 128,
      pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
      description:
        "Opaque ASCII correlation identifier (1–128 characters); never an authentication credential.",
    },
    Counter: {
      type: "integer",
      minimum: 0,
      maximum: 9007199254740991,
      description:
        "Nonnegative integer in the shared JavaScript safe-integer range.",
    },
    Namespace: {
      type: "object",
      properties: {
        tenantId: {
          $ref: "#/$defs/Id",
          description: "Authenticated tenant scope.",
        },
        principalId: {
          $ref: "#/$defs/Id",
          description: "Authenticated principal scope.",
        },
        authorityId: {
          $ref: "#/$defs/Id",
          description: "Authenticated authority scope.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Logical session identifier, never reusable after retirement.",
        },
      },
      required: ["tenantId", "principalId", "authorityId", "sessionId"],
      additionalProperties: false,
      description:
        "Trusted tenant/principal/authority/logical-session storage scope supplied by authenticated ingress.",
    },
    ConfigRef: {
      type: "object",
      properties: {
        id: {
          $ref: "#/$defs/Id",
          description:
            "Configuration identifier; resolve credentials outside the wire.",
        },
        revision: {
          $ref: "#/$defs/Id",
          description:
            "Immutable configuration revision; changing it invalidates prior capability evidence.",
        },
      },
      required: ["id", "revision"],
      additionalProperties: false,
      description:
        "Immutable configuration identity and revision; contains no credentials.",
    },
    Binding: {
      type: "object",
      properties: {
        provider: {
          $ref: "#/$defs/Id",
          description: "Provider adapter identity.",
        },
        providerVersion: {
          $ref: "#/$defs/Id",
          description: "Pinned native provider implementation version.",
        },
        adapterVersion: {
          $ref: "#/$defs/Id",
          description:
            "Adapter implementation version used for capability verification.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        accountRef: {
          $ref: "#/$defs/Id",
          description:
            "Opaque account reference; no token, key or account-directory contents.",
        },
        nativeSessionId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned context session identifier; history alone cannot recreate it.",
        },
        config: {
          $ref: "#/$defs/ConfigRef",
          description: "Exact immutable configuration identity and revision.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
        nativeRequestId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction.",
        },
      },
      required: [
        "provider",
        "providerVersion",
        "adapterVersion",
        "generation",
        "accountRef",
        "nativeSessionId",
        "config",
      ],
      additionalProperties: false,
      description:
        "Provider context identity. Version, configuration, account and generation bind every capability and callback.",
    },
    CapabilityState: {
      type: "string",
      enum: ["supported", "unsupported", "unknown"],
      description:
        "Only supported enables an operation; unknown and unsupported fail closed.",
    },
    Capabilities: {
      type: "object",
      properties: {
        continuation: {
          type: "string",
          enum: ["same_process", "across_processes", "unsupported", "unknown"],
          description:
            "same_process resumes only a live context; across_processes requires verified provider restoration; unsupported/unknown cannot resume.",
        },
        cancellation: {
          type: "string",
          enum: [
            "request_only",
            "terminal_acknowledged",
            "unsupported",
            "unknown",
          ],
          description:
            "request_only confirms sending; terminal_acknowledged requires native terminal evidence; unsupported/unknown cannot promise cancellation.",
        },
        tools: {
          type: "string",
          enum: ["host_mediated", "provider_managed", "disabled", "unknown"],
          description:
            "host_mediated still requires containment evidence; provider_managed is not controlled execution; disabled/unknown cannot enable tools.",
        },
        steer: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether an active native run accepts targeted steering.",
        },
        fork: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether the provider supports an explicit context fork.",
        },
        subagent: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether the provider supports child agents; not execution authorization.",
        },
        terminal: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether the provider exposes a terminal facility; not the command terminal state.",
        },
        structuredQuestion: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether a native structured callback can be represented and answered.",
        },
        multimodal: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether provider-specific multimodal input is available through an adapter extension.",
        },
        queue: {
          $ref: "#/$defs/CapabilityState",
          description:
            "Whether additional prompts may be queued while a run is active.",
        },
      },
      required: [
        "continuation",
        "cancellation",
        "tools",
        "steer",
        "fork",
        "subagent",
        "terminal",
        "structuredQuestion",
        "multimodal",
        "queue",
      ],
      additionalProperties: false,
      description:
        "Capabilities established for one exact provider binding, never execution authorization.",
    },
    CommandState: {
      type: "string",
      enum: [
        "accepted",
        "dispatching",
        "running",
        "terminal",
        "reconciliation_required",
      ],
      description:
        "accepted persists intent; dispatching persists dispatch intent; running has native confirmation; terminal has a definite outcome; reconciliation_required forbids blind resubmission.",
    },
    Outcome: {
      type: "string",
      enum: [
        "completed",
        "cancelled",
        "refused",
        "max_tokens",
        "max_turn_requests",
        "failed",
      ],
      description:
        "Definite model-turn outcome; does not establish process exit or business-side-effect completion.",
    },
    ErrorCode: {
      type: "string",
      enum: [
        "invalid_input",
        "unsupported_version",
        "unsupported_capability",
        "permission_denied",
        "content_conflict",
        "revision_conflict",
        "stale_binding",
        "expired",
        "unavailable",
        "reconciliation_required",
        "limit_exceeded",
        "cursor_expired",
        "session_gone",
        "already_answered",
      ],
      description:
        "Closed value-free error category; diagnostics never include model text or credentials.",
    },
    Retry: {
      type: "string",
      enum: ["same_command", "reconcile_first", "never"],
      description:
        "same_command preserves identity/content; reconcile_first checks the original operation; never forbids retry.",
    },
    Failure: {
      type: "object",
      properties: {
        code: {
          $ref: "#/$defs/ErrorCode",
          description: "Closed diagnostic category without input values.",
        },
        retry: {
          $ref: "#/$defs/Retry",
          description:
            "Explicit retry discipline; uncertainty never authorizes blind resubmission.",
        },
      },
      required: ["code", "retry"],
      additionalProperties: false,
      description: "Value-free failure and explicit retry discipline.",
    },
    Input: {
      oneOf: [
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "prompt",
              description: "Closed variant discriminator.",
            },
            text: {
              type: "string",
              maxLength: 65536,
              description:
                "Untrusted model/user text subject to the whole-envelope budgets.",
            },
            policy: {
              type: "string",
              enum: ["queue_next", "steer"],
              description:
                "queue_next serializes later work; steer must match the currently active native run.",
            },
            targetRunId: {
              $ref: "#/$defs/Id",
              description:
                "Exact active native run required for steer; forbidden for queue_next.",
            },
          },
          required: ["type", "text", "policy"],
          additionalProperties: false,
          description:
            "prompt variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "cancel",
              description: "Closed variant discriminator.",
            },
            targetCommandId: {
              $ref: "#/$defs/Id",
              description: "Original accepted command being cancelled.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            nativeRunId: {
              $ref: "#/$defs/Id",
              description:
                "Provider-owned model-turn/run identifier, required when the provider exposes it.",
            },
          },
          required: ["type", "targetCommandId", "generation"],
          additionalProperties: false,
          description:
            "cancel variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "respond",
              description: "Closed variant discriminator.",
            },
            interactionId: {
              $ref: "#/$defs/Id",
              description:
                "Single-use interaction identity within the namespace.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            nativeRunId: {
              $ref: "#/$defs/Id",
              description:
                "Provider-owned model-turn/run identifier, required when the provider exposes it.",
            },
            answer: {
              type: "object",
              additionalProperties: true,
              description:
                "Untrusted JSON response data; cannot carry approval authority.",
            },
            surface: {
              $ref: "#/$defs/SurfaceReference",
              description:
                "Mandatory for an interaction associated with a surface; cannot bypass deleted/stale state.",
            },
          },
          required: ["type", "interactionId", "generation", "answer"],
          additionalProperties: false,
          description:
            "respond variant; all fields are data, never authentication or execution authority.",
        },
      ],
      description:
        "Product command inputs; provider-specific formats remain adapter-owned.",
    },
    Command: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "command",
          description: "Closed product record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Logical session identifier, never reusable after retirement.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description:
            "Client-generated idempotency key; reuse only with identical canonical content.",
        },
        expiresAtMs: {
          $ref: "#/$defs/Counter",
          description:
            "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
        },
        input: {
          $ref: "#/$defs/Input",
          description:
            "Complete command content included in its canonical fingerprint.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "sessionId",
        "commandId",
        "expiresAtMs",
        "input",
      ],
      additionalProperties: false,
      description:
        "Client command identity and complete canonical input; trusted namespace is supplied separately.",
    },
    Receipt: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "receipt",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description:
            "Client-generated idempotency key; reuse only with identical canonical content.",
        },
        contentHash: {
          type: "string",
          pattern: "^[a-f0-9]{64}$",
          description:
            "SHA-256 of JCS(command), including every command field; namespace is a separate storage key.",
        },
        acceptedAtMs: {
          $ref: "#/$defs/Counter",
          description: "UTC epoch milliseconds at committed acceptance.",
        },
        retryUntilMs: {
          $ref: "#/$defs/Counter",
          description:
            "Inclusive same-command retry deadline, no later than command expiry.",
        },
        receiptUntilMs: {
          $ref: "#/$defs/Counter",
          description:
            "Inclusive stored-receipt deadline; cannot be shorter than the retry deadline.",
        },
        acceptedRevision: {
          $ref: "#/$defs/Counter",
          description:
            "Session revision which atomically accepted this command.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "commandId",
        "contentHash",
        "acceptedAtMs",
        "retryUntilMs",
        "receiptUntilMs",
        "acceptedRevision",
      ],
      additionalProperties: false,
      description:
        "Immutable acceptance fact. Only an actual committed store makes it durable; it is not a model terminal.",
    },
    Dispatch: {
      type: "object",
      properties: {
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeSessionId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned context session identifier; history alone cannot recreate it.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
        nativeRequestId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction.",
        },
        certainty: {
          type: "string",
          enum: ["not_sent", "submitted", "unknown"],
          description:
            "submitted has native acceptance; unknown requires reconciliation; not_sent has evidence no submission occurred.",
        },
      },
      required: ["generation", "nativeSessionId", "certainty"],
      additionalProperties: false,
      description:
        "Persisted native correlation and certainty; unknown requires reconciliation before any further send.",
    },
    CommandRecord: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "commandRecord",
          description: "Closed product record discriminator.",
        },
        command: {
          $ref: "#/$defs/Command",
          description: "Immutable original command.",
        },
        receipt: {
          $ref: "#/$defs/Receipt",
          description: "Immutable original committed acceptance fact.",
        },
        state: {
          $ref: "#/$defs/CommandState",
          description:
            "Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence.",
        },
        dispatch: {
          $ref: "#/$defs/Dispatch",
          description:
            "Native correlation persisted before/with dispatch; never reconstructed from UI history.",
        },
        outcome: {
          $ref: "#/$defs/Outcome",
          description:
            "Definite model-turn result; no implication about business side effects.",
        },
        failure: {
          $ref: "#/$defs/Failure",
          description: "Closed failure category and retry discipline.",
        },
      },
      required: ["schemaVersion", "kind", "command", "receipt", "state"],
      additionalProperties: false,
      description:
        "Single inbox/dispatch ledger; no second provider queue owns the same command.",
    },
    EventBody: {
      oneOf: [
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "text",
              description: "Closed variant discriminator.",
            },
            messageId: {
              $ref: "#/$defs/Id",
              description: "Stable product message correlation identifier.",
            },
            text: {
              type: "string",
              maxLength: 65536,
              description:
                "Untrusted model/user text subject to the whole-envelope budgets.",
            },
          },
          required: ["type", "messageId", "text"],
          additionalProperties: false,
          description:
            "text variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "status",
              description: "Closed variant discriminator.",
            },
            state: {
              $ref: "#/$defs/CommandState",
              description:
                "Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence.",
            },
          },
          required: ["type", "state"],
          additionalProperties: false,
          description:
            "status variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "terminal",
              description: "Closed variant discriminator.",
            },
            outcome: {
              $ref: "#/$defs/Outcome",
              description:
                "Definite model-turn result; no implication about business side effects.",
            },
          },
          required: ["type", "outcome"],
          additionalProperties: false,
          description:
            "terminal variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "cancel_dispatched",
              description: "Closed variant discriminator.",
            },
            confirmation: {
              type: "string",
              enum: ["request_only", "already_terminal", "unsupported"],
              description:
                "Cancellation request transport confirmation only; does not manufacture a model terminal.",
            },
          },
          required: ["type", "confirmation"],
          additionalProperties: false,
          description:
            "cancel_dispatched variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "tool_proposal",
              description: "Closed variant discriminator.",
            },
            proposalId: {
              $ref: "#/$defs/Id",
              description: "Untrusted tool proposal correlation identifier.",
            },
            name: {
              $ref: "#/$defs/Id",
              description:
                "Provider tool name; not an approved execution action.",
            },
            arguments: {
              type: "object",
              additionalProperties: true,
              description:
                "Untrusted tool JSON arguments, including keys, count toward product budgets.",
            },
          },
          required: ["type", "proposalId", "name", "arguments"],
          additionalProperties: false,
          description:
            "tool_proposal variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "tool_result",
              description: "Closed variant discriminator.",
            },
            proposalId: {
              $ref: "#/$defs/Id",
              description: "Untrusted tool proposal correlation identifier.",
            },
            disposition: {
              type: "string",
              enum: ["returned", "rejected", "unavailable"],
              description:
                "Protocol tool-result disposition, not authoritative business execution status.",
            },
            text: {
              type: "string",
              maxLength: 65536,
              description:
                "Untrusted model/user text subject to the whole-envelope budgets.",
            },
          },
          required: ["type", "proposalId", "disposition", "text"],
          additionalProperties: false,
          description:
            "tool_result variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "interaction",
              description: "Closed variant discriminator.",
            },
            interactionId: {
              $ref: "#/$defs/Id",
              description:
                "Single-use interaction identity within the namespace.",
            },
            status: {
              type: "string",
              const: "pending",
              description: "First publication of an ordinary user question.",
            },
            request: {
              $ref: "#/$defs/InteractionRequest",
              description:
                "Required for the first pending event and equal to the newly committed Interaction request; forbidden on later lifecycle events.",
            },
            expiresAtMs: {
              $ref: "#/$defs/Counter",
              description:
                "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
            },
            callbackLifetime: {
              $ref: "#/$defs/CallbackLifetime",
              description:
                "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
            },
          },
          required: [
            "type",
            "interactionId",
            "status",
            "request",
            "expiresAtMs",
            "callbackLifetime",
          ],
          additionalProperties: false,
          description:
            "Initial ordinary question publication; the matching Interaction is committed atomically.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "interaction",
              description: "Closed variant discriminator.",
            },
            interactionId: {
              $ref: "#/$defs/Id",
              description:
                "Single-use interaction identity within the namespace.",
            },
            status: {
              type: "string",
              const: "answered",
              description:
                "The first accepted response consumed this interaction.",
            },
            responseCommandId: {
              $ref: "#/$defs/Id",
              description:
                "Accepted response command which atomically consumed the interaction; present only when answered.",
            },
          },
          required: ["type", "interactionId", "status", "responseCommandId"],
          additionalProperties: false,
          description:
            "First accepted response identity, committed atomically with the receipt and Interaction.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "interaction",
              description: "Closed variant discriminator.",
            },
            interactionId: {
              $ref: "#/$defs/Id",
              description:
                "Single-use interaction identity within the namespace.",
            },
            status: {
              type: "string",
              enum: ["expired", "unavailable"],
              description:
                "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
            },
          },
          required: ["type", "interactionId", "status"],
          additionalProperties: false,
          description:
            "Question lifecycle transition; cannot republish or replace its request.",
        },
        {
          type: "object",
          properties: {
            type: {
              type: "string",
              const: "error",
              description: "Closed variant discriminator.",
            },
            failure: {
              $ref: "#/$defs/Failure",
              description: "Closed failure category and retry discipline.",
            },
          },
          required: ["type", "failure"],
          additionalProperties: false,
          description:
            "error variant; all fields are data, never authentication or execution authority.",
        },
        {
          type: "object",
          properties: {
            type: {
              const: "surface",
              type: "string",
              description: "Closed variant discriminator.",
            },
            surface: {
              $ref: "#/$defs/SurfaceState",
              description:
                "Full surface recovery state committed with this event.",
            },
          },
          required: ["type", "surface"],
          additionalProperties: false,
        },
      ],
      description:
        "Stable product observation. Tool proposals and results are untrusted and cannot issue execution authority.",
    },
    Event: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "event",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        eventId: {
          $ref: "#/$defs/Id",
          description: "Stable unique event identifier within the namespace.",
        },
        sequence: {
          $ref: "#/$defs/Counter",
          description:
            "Strictly increasing stable-event counter; attach cursors are exclusive.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description:
            "Client-generated idempotency key; reuse only with identical canonical content.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        body: {
          $ref: "#/$defs/EventBody",
          description: "Stable observation committed before publication.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "eventId",
        "sequence",
        "commandId",
        "generation",
        "body",
      ],
      additionalProperties: false,
      description:
        "Committed stable event in one namespace; token deltas are excluded from durable ordering.",
    },
    Session: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "session",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        revision: {
          $ref: "#/$defs/Counter",
          description: "Monotonic CAS revision of this product record.",
        },
        lastSequence: {
          $ref: "#/$defs/Counter",
          description:
            "Highest committed stable-event sequence at this session revision.",
        },
        binding: {
          $ref: "#/$defs/Binding",
          description:
            "Exact provider incarnation and native context identity.",
        },
        capabilities: {
          $ref: "#/$defs/Capabilities",
          description:
            "Capabilities bound to this exact provider/configuration/account incarnation.",
        },
        status: {
          type: "string",
          enum: ["active", "retired"],
          description:
            "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "revision",
        "lastSequence",
        "binding",
        "capabilities",
        "status",
      ],
      additionalProperties: false,
      description:
        "Logical session state and stable event watermark committed at one revision.",
    },
    Interaction: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "interaction",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description:
            "Client-generated idempotency key; reuse only with identical canonical content.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
        expiresAtMs: {
          $ref: "#/$defs/Counter",
          description:
            "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
        },
        status: {
          type: "string",
          enum: ["pending", "answered", "expired", "unavailable"],
          description:
            "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
        },
        responseCommandId: {
          $ref: "#/$defs/Id",
          description:
            "Accepted response command which atomically consumed the interaction; present only when answered.",
        },
        callbackLifetime: {
          $ref: "#/$defs/CallbackLifetime",
          description:
            "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
        },
        nativeCallbackId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned callback identifier, immutable and unique within a session generation; distinct from the parent dispatch request.",
        },
        request: {
          $ref: "#/$defs/InteractionRequest",
          description:
            "Immutable untrusted question payload retained for display; does not restore a lost native callback or grant approval.",
        },
        category: {
          type: "string",
          const: "question",
          description:
            "Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "interactionId",
        "commandId",
        "generation",
        "nativeCallbackId",
        "expiresAtMs",
        "status",
        "callbackLifetime",
        "request",
        "category",
      ],
      additionalProperties: false,
      description:
        "Single-use provider callback with immutable command/native correlation, expiry and lifetime.",
    },
    Delivery: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "delivery",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        operationId: {
          $ref: "#/$defs/Id",
          description:
            "Stable receiver idempotency key for this delivery; cannot be rebound to another event/target.",
        },
        eventId: {
          $ref: "#/$defs/Id",
          description: "Stable unique event identifier within the namespace.",
        },
        target: {
          $ref: "#/$defs/Id",
          description: "Opaque reliable-delivery destination identifier.",
        },
        contentHash: {
          type: "string",
          pattern: "^[a-f0-9]{64}$",
          description:
            "SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination.",
        },
        retry: {
          type: "string",
          enum: ["receiver_idempotent", "reconcile_first", "never"],
          description:
            "Explicit retry discipline; uncertainty never authorizes blind resubmission.",
        },
        status: {
          type: "string",
          enum: ["pending", "delivered", "reconciliation_required"],
          description:
            "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
        },
        attempts: {
          $ref: "#/$defs/Counter",
          description: "Monotonic count of delivery attempts.",
        },
        nextAttemptAtMs: {
          $ref: "#/$defs/Counter",
          description: "UTC epoch-millisecond earliest eligible retry time.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "operationId",
        "eventId",
        "target",
        "contentHash",
        "retry",
        "status",
        "attempts",
        "nextAttemptAtMs",
      ],
      additionalProperties: false,
      description:
        "Reliable cross-service outbox record bound to an immutable event and target.",
    },
    SurfaceAction: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "surfaceAction",
          description: "Closed product record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Logical session identifier, never reusable after retirement.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description:
            "Client-generated idempotency key; reuse only with identical canonical content.",
        },
        surfaceInstanceId: {
          $ref: "#/$defs/Id",
          description:
            "Fresh product identity for each surface creation; deletion permanently invalidates old actions.",
        },
        surfaceRevision: {
          $ref: "#/$defs/Counter",
          description:
            "Exact current surface revision required to accept this action.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "sessionId",
        "commandId",
        "surfaceInstanceId",
        "surfaceRevision",
        "interactionId",
        "generation",
        "nativeRunId",
      ],
      additionalProperties: false,
      description:
        "Product metadata accompanying an unchanged upstream action; association does not grant permission.",
    },
    SurfaceReference: {
      type: "object",
      properties: {
        instanceId: {
          $ref: "#/$defs/Id",
          description:
            "Exact product surface instance associated with this response.",
        },
        revision: {
          $ref: "#/$defs/Counter",
          description:
            "Current surface revision checked atomically during response acceptance.",
        },
      },
      required: ["instanceId", "revision"],
      additionalProperties: false,
      description:
        "Surface revision checked atomically when accepting an action response.",
    },
    InteractionRequest: {
      type: "object",
      additionalProperties: true,
      description:
        "Immutable untrusted provider question payload. Subject to whole-record JSON budgets; never authentication, permission or execution approval.",
    },
    SurfaceState: {
      type: "object",
      properties: {
        schemaVersion: {
          type: "integer",
          const: 2,
          description:
            "Exact product wire version; V1 is rejected without migration or fallback.",
        },
        kind: {
          type: "string",
          const: "surface",
          description: "Closed product record discriminator.",
        },
        namespace: {
          $ref: "#/$defs/Namespace",
          description:
            "Trusted storage isolation scope; not copied from model or action content.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
        surfaceId: {
          $ref: "#/$defs/Id",
          description: "Upstream A2UI surface identifier.",
        },
        surfaceInstanceId: {
          $ref: "#/$defs/Id",
          description:
            "Fresh product identity for each surface creation; deletion permanently invalidates old actions.",
        },
        revision: {
          $ref: "#/$defs/Counter",
          description: "Monotonic CAS revision of this product record.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        sourceComponentId: {
          $ref: "#/$defs/Id",
          description:
            "Exact upstream source component allowed to emit this action.",
        },
        eventName: {
          $ref: "#/$defs/Id",
          description:
            "Exact upstream action name associated with this interaction.",
        },
        catalogId: {
          $ref: "#/$defs/Id",
          description: "Negotiated upstream catalog identity.",
        },
        catalogVersion: {
          $ref: "#/$defs/Id",
          description:
            "Negotiated fixed catalog version; not a renderer implementation claim.",
        },
        a2uiVersion: {
          type: "string",
          const: "v0.9.1",
          description: "Exact negotiated upstream A2UI version.",
        },
        status: {
          type: "string",
          enum: ["active", "deleted"],
          description:
            "Persisted lifecycle; deleted is a permanent tombstone for this instance.",
        },
        messages: {
          type: "array",
          maxItems: 128,
          items: { type: "object", additionalProperties: true },
          description:
            "Bounded unchanged upstream messages needed to rebuild this instance; not a second A2UI schema.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "namespace",
        "generation",
        "nativeRunId",
        "surfaceId",
        "surfaceInstanceId",
        "revision",
        "interactionId",
        "sourceComponentId",
        "eventName",
        "catalogId",
        "catalogVersion",
        "a2uiVersion",
        "status",
        "messages",
      ],
      additionalProperties: false,
      description:
        "Single surface record: association, lifecycle and bounded upstream recovery content.",
    },
    PageQuery: {
      type: "object",
      properties: {
        limit: {
          type: "integer",
          minimum: 1,
          maximum: 256,
          description: "Maximum records in this page, from 1 to 256.",
        },
        continuation: {
          $ref: "#/$defs/Id",
          description:
            "Opaque continuation of one immutable read view; expires independently of the session.",
        },
      },
      required: ["limit"],
      additionalProperties: false,
    },
    SnapshotPage: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "snapshotPage",
          type: "string",
          description: "Closed record discriminator.",
        },
        snapshotId: {
          $ref: "#/$defs/Id",
          description:
            "Identity shared by all pages from one immutable read view.",
        },
        pageIndex: {
          $ref: "#/$defs/Counter",
          description: "Zero-based page order within this snapshot.",
        },
        session: {
          $ref: "#/$defs/Session",
          description: "Session at the snapshot watermark.",
        },
        cursor: {
          $ref: "#/$defs/Counter",
          description: "Stable event watermark shared by every page.",
        },
        events: {
          type: "array",
          items: { $ref: "#/$defs/Event" },
          description: "Stable events at or below the watermark.",
        },
        commands: {
          type: "array",
          items: { $ref: "#/$defs/CommandRecord" },
          description: "Command projections at the watermark.",
        },
        interactions: {
          type: "array",
          items: { $ref: "#/$defs/Interaction" },
          description:
            "Interaction display state; does not restore a native callback.",
        },
        surfaces: {
          type: "array",
          items: { $ref: "#/$defs/SurfaceState" },
          description:
            "Bounded original A2UI recovery messages and their associations.",
        },
        next: {
          $ref: "#/$defs/Id",
          description: "Opaque continuation; absent at end of the read view.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "snapshotId",
        "pageIndex",
        "session",
        "cursor",
        "events",
        "commands",
        "interactions",
        "surfaces",
      ],
      additionalProperties: false,
    },
    SessionPage: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "sessionPage",
          type: "string",
          description: "Closed record discriminator.",
        },
        items: {
          type: "array",
          items: { $ref: "#/$defs/Session" },
          description: "Caller-scoped sessions in this immutable page.",
        },
        next: {
          $ref: "#/$defs/Id",
          description: "Opaque continuation; absent at end of the read view.",
        },
      },
      required: ["schemaVersion", "kind", "items"],
      additionalProperties: false,
    },
    Subscription: {
      oneOf: [
        {
          type: "object",
          properties: {
            type: {
              const: "event",
              type: "string",
              description: "Closed variant discriminator.",
            },
            event: { $ref: "#/$defs/Event", description: "Stable Host event." },
          },
          required: ["type", "event"],
          additionalProperties: false,
        },
        {
          type: "object",
          properties: {
            type: {
              const: "delta",
              type: "string",
              description: "Closed variant discriminator.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description: "Exact native provider incarnation.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description: "Product command identity.",
            },
            messageId: {
              $ref: "#/$defs/Id",
              description: "Identity of the streamed message within a command.",
            },
            text: { type: "string", description: "Untrusted display text." },
          },
          required: ["type", "generation", "commandId", "messageId", "text"],
          additionalProperties: false,
        },
        {
          type: "object",
          properties: {
            type: {
              const: "resync_required",
              type: "string",
              description: "Closed variant discriminator.",
            },
          },
          required: ["type"],
          additionalProperties: false,
        },
      ],
    },
    SnapshotRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "snapshotRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
        query: {
          $ref: "#/$defs/PageQuery",
          description:
            "Bounded page query with an opaque caller-bound continuation.",
        },
      },
      required: ["schemaVersion", "kind", "sessionId", "query"],
      additionalProperties: false,
    },
    ListRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "listRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        query: {
          $ref: "#/$defs/PageQuery",
          description:
            "Bounded page query with an opaque caller-bound continuation.",
        },
      },
      required: ["schemaVersion", "kind", "query"],
      additionalProperties: false,
    },
    AttachRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "attachRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
        after: {
          $ref: "#/$defs/Counter",
          description: "Last stable sequence consumed before attaching.",
        },
        attachmentId: {
          $ref: "#/$defs/Id",
          description:
            "Connection-local attachment identity, echoed on every update.",
        },
      },
      required: ["schemaVersion", "kind", "sessionId", "after", "attachmentId"],
      additionalProperties: false,
    },
    DetachRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "detachRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
        attachmentId: {
          $ref: "#/$defs/Id",
          description:
            "Connection-local attachment identity, echoed on every update.",
        },
      },
      required: ["schemaVersion", "kind", "sessionId", "attachmentId"],
      additionalProperties: false,
    },
    ResumeRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "resumeRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
      },
      required: ["schemaVersion", "kind", "sessionId"],
      additionalProperties: false,
    },
    ActionRequest: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "actionRequest",
          type: "string",
          description: "Closed record discriminator.",
        },
        metadata: {
          $ref: "#/$defs/SurfaceAction",
          description:
            "Product action association, checked independently of untrusted upstream context.",
        },
        message: {
          type: "object",
          additionalProperties: true,
          description:
            "Unchanged upstream A2UI client message, validated against the negotiated schema.",
        },
        expiresAtMs: {
          $ref: "#/$defs/Counter",
          description: "Command expiry in Unix milliseconds.",
        },
      },
      required: ["schemaVersion", "kind", "metadata", "message", "expiresAtMs"],
      additionalProperties: false,
    },
    AccessUpdate: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "accessUpdate",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
        attachmentId: {
          $ref: "#/$defs/Id",
          description:
            "Connection-local attachment identity, echoed on every update.",
        },
        update: {
          $ref: "#/$defs/Subscription",
          description:
            "Stable event, ephemeral delta, or explicit resynchronization signal.",
        },
      },
      required: [
        "schemaVersion",
        "kind",
        "sessionId",
        "attachmentId",
        "update",
      ],
      additionalProperties: false,
    },
    AttachReceipt: {
      type: "object",
      properties: {
        schemaVersion: {
          const: 2,
          type: "integer",
          description: "Exact product contract version; no legacy readers.",
        },
        kind: {
          const: "attachReceipt",
          type: "string",
          description: "Closed record discriminator.",
        },
        sessionId: {
          $ref: "#/$defs/Id",
          description:
            "Product session identity within the authenticated caller namespace.",
        },
        attachmentId: {
          $ref: "#/$defs/Id",
          description:
            "Connection-local attachment identity, echoed on every update.",
        },
        after: {
          $ref: "#/$defs/Counter",
          description: "Last stable sequence consumed before attaching.",
        },
      },
      required: ["schemaVersion", "kind", "sessionId", "attachmentId", "after"],
      additionalProperties: false,
    },
    Negotiation: {
      description:
        "Selected product ACP extensions; capability metadata is never execution authority.",
      type: "object",
      additionalProperties: false,
      properties: {
        contractVersion: {
          description: "Exact product contract version.",
          type: "integer",
          const: 2,
        },
        acp: {
          description: "Exact ACP protocol version.",
          type: "integer",
          const: 1,
        },
        durableReceipts: {
          description:
            "Host supports transactional receipt semantics; memory doubles simulate this only.",
          type: "boolean",
        },
        cursorAttach: {
          description: "Host supports stable snapshot-to-event attachment.",
          type: "boolean",
        },
        a2ui: {
          $ref: "#/$defs/A2uiNegotiation",
          description:
            "Explicitly selected upstream version and product catalog.",
        },
      },
      required: ["contractVersion", "acp", "durableReceipts", "cursorAttach"],
    },
    A2uiNegotiation: {
      description: "Explicitly selected upstream version and product catalog.",
      type: "object",
      additionalProperties: false,
      properties: {
        version: {
          description: "Fixed upstream protocol version.",
          type: "string",
          const: "v0.9.1",
        },
        catalogId: {
          type: "string",
          const: "urn:rss-mdm-agent:a2ui:interaction",
          description: "Fixed product catalog identity.",
        },
        catalogVersion: {
          type: "string",
          const: "1",
          description: "Fixed product catalog revision.",
        },
      },
      required: ["version", "catalogId", "catalogVersion"],
    },
    CallbackLifetime: {
      type: "string",
      enum: ["generation_bound", "provider_resumable"],
      description:
        "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
    },
  },
};
const schema32 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "command",
      description: "Closed product record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Logical session identifier, never reusable after retirement.",
    },
    commandId: {
      $ref: "#/$defs/Id",
      description:
        "Client-generated idempotency key; reuse only with identical canonical content.",
    },
    expiresAtMs: {
      $ref: "#/$defs/Counter",
      description:
        "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
    },
    input: {
      $ref: "#/$defs/Input",
      description:
        "Complete command content included in its canonical fingerprint.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "sessionId",
    "commandId",
    "expiresAtMs",
    "input",
  ],
  additionalProperties: false,
  description:
    "Client command identity and complete canonical input; trusted namespace is supplied separately.",
};
const schema33 = {
  type: "string",
  minLength: 1,
  maxLength: 128,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
  description:
    "Opaque ASCII correlation identifier (1–128 characters); never an authentication credential.",
};
const schema35 = {
  type: "integer",
  minimum: 0,
  maximum: 9007199254740991,
  description:
    "Nonnegative integer in the shared JavaScript safe-integer range.",
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/+-]*$", "u");
const schema36 = {
  oneOf: [
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "prompt",
          description: "Closed variant discriminator.",
        },
        text: {
          type: "string",
          maxLength: 65536,
          description:
            "Untrusted model/user text subject to the whole-envelope budgets.",
        },
        policy: {
          type: "string",
          enum: ["queue_next", "steer"],
          description:
            "queue_next serializes later work; steer must match the currently active native run.",
        },
        targetRunId: {
          $ref: "#/$defs/Id",
          description:
            "Exact active native run required for steer; forbidden for queue_next.",
        },
      },
      required: ["type", "text", "policy"],
      additionalProperties: false,
      description:
        "prompt variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "cancel",
          description: "Closed variant discriminator.",
        },
        targetCommandId: {
          $ref: "#/$defs/Id",
          description: "Original accepted command being cancelled.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
      },
      required: ["type", "targetCommandId", "generation"],
      additionalProperties: false,
      description:
        "cancel variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "respond",
          description: "Closed variant discriminator.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description:
            "Live provider incarnation token; rejects callbacks from previous incarnations.",
        },
        nativeRunId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned model-turn/run identifier, required when the provider exposes it.",
        },
        answer: {
          type: "object",
          additionalProperties: true,
          description:
            "Untrusted JSON response data; cannot carry approval authority.",
        },
        surface: {
          $ref: "#/$defs/SurfaceReference",
          description:
            "Mandatory for an interaction associated with a surface; cannot bypass deleted/stale state.",
        },
      },
      required: ["type", "interactionId", "generation", "answer"],
      additionalProperties: false,
      description:
        "respond variant; all fields are data, never authentication or execution authority.",
    },
  ],
  description:
    "Product command inputs; provider-specific formats remain adapter-owned.",
};
const schema44 = {
  type: "object",
  properties: {
    instanceId: {
      $ref: "#/$defs/Id",
      description:
        "Exact product surface instance associated with this response.",
    },
    revision: {
      $ref: "#/$defs/Counter",
      description:
        "Current surface revision checked atomically during response acceptance.",
    },
  },
  required: ["instanceId", "revision"],
  additionalProperties: false,
  description:
    "Surface revision checked atomically when accepting an action response.",
};
function validate23(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate23.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.instanceId === undefined && (missing0 = "instanceId")) ||
        (data.revision === undefined && (missing0 = "revision"))
      ) {
        validate23.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "instanceId" || key0 === "revision")) {
            validate23.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.instanceId !== undefined) {
            let data0 = data.instanceId;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate23.errors = [
                    {
                      instancePath: instancePath + "/instanceId",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate23.errors = [
                      {
                        instancePath: instancePath + "/instanceId",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate23.errors = [
                        {
                          instancePath: instancePath + "/instanceId",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate23.errors = [
                  {
                    instancePath: instancePath + "/instanceId",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.revision !== undefined) {
              let data1 = data.revision;
              const _errs5 = errors;
              const _errs6 = errors;
              if (
                !(
                  typeof data1 == "number" &&
                  !(data1 % 1) &&
                  !isNaN(data1) &&
                  isFinite(data1)
                )
              ) {
                validate23.errors = [
                  {
                    instancePath: instancePath + "/revision",
                    schemaPath: "#/$defs/Counter/type",
                    keyword: "type",
                    params: { type: "integer" },
                    message: "must be integer",
                  },
                ];
                return false;
              }
              if (errors === _errs6) {
                if (typeof data1 == "number" && isFinite(data1)) {
                  if (data1 > 9007199254740991 || isNaN(data1)) {
                    validate23.errors = [
                      {
                        instancePath: instancePath + "/revision",
                        schemaPath: "#/$defs/Counter/maximum",
                        keyword: "maximum",
                        params: { comparison: "<=", limit: 9007199254740991 },
                        message: "must be <= 9007199254740991",
                      },
                    ];
                    return false;
                  } else {
                    if (data1 < 0 || isNaN(data1)) {
                      validate23.errors = [
                        {
                          instancePath: instancePath + "/revision",
                          schemaPath: "#/$defs/Counter/minimum",
                          keyword: "minimum",
                          params: { comparison: ">=", limit: 0 },
                          message: "must be >= 0",
                        },
                      ];
                      return false;
                    }
                  }
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate23.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate23.errors = vErrors;
  return errors === 0;
}
validate23.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate22(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate22.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.type === undefined && (missing0 = "type")) ||
        (data.text === undefined && (missing0 = "text")) ||
        (data.policy === undefined && (missing0 = "policy"))
      ) {
        const err0 = {
          instancePath,
          schemaPath: "#/oneOf/0/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "type" ||
              key0 === "text" ||
              key0 === "policy" ||
              key0 === "targetRunId"
            )
          ) {
            const err1 = {
              instancePath,
              schemaPath: "#/oneOf/0/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.type !== undefined) {
            let data0 = data.type;
            const _errs4 = errors;
            if (typeof data0 !== "string") {
              const err2 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("prompt" !== data0) {
              const err3 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/const",
                keyword: "const",
                params: { allowedValue: "prompt" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.text !== undefined) {
              let data1 = data.text;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 65536) {
                    const err4 = {
                      instancePath: instancePath + "/text",
                      schemaPath: "#/oneOf/0/properties/text/maxLength",
                      keyword: "maxLength",
                      params: { limit: 65536 },
                      message: "must NOT have more than 65536 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  }
                } else {
                  const err5 = {
                    instancePath: instancePath + "/text",
                    schemaPath: "#/oneOf/0/properties/text/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err5];
                  } else {
                    vErrors.push(err5);
                  }
                  errors++;
                }
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
            if (valid1) {
              if (data.policy !== undefined) {
                let data2 = data.policy;
                const _errs8 = errors;
                if (typeof data2 !== "string") {
                  const err6 = {
                    instancePath: instancePath + "/policy",
                    schemaPath: "#/oneOf/0/properties/policy/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                }
                if (!(data2 === "queue_next" || data2 === "steer")) {
                  const err7 = {
                    instancePath: instancePath + "/policy",
                    schemaPath: "#/oneOf/0/properties/policy/enum",
                    keyword: "enum",
                    params: {
                      allowedValues: schema36.oneOf[0].properties.policy.enum,
                    },
                    message: "must be equal to one of the allowed values",
                  };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
                var valid1 = _errs8 === errors;
              } else {
                var valid1 = true;
              }
              if (valid1) {
                if (data.targetRunId !== undefined) {
                  let data3 = data.targetRunId;
                  const _errs10 = errors;
                  const _errs11 = errors;
                  if (errors === _errs11) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        const err8 = {
                          instancePath: instancePath + "/targetRunId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err8];
                        } else {
                          vErrors.push(err8);
                        }
                        errors++;
                      } else {
                        if (func1(data3) < 1) {
                          const err9 = {
                            instancePath: instancePath + "/targetRunId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err9];
                          } else {
                            vErrors.push(err9);
                          }
                          errors++;
                        } else {
                          if (!pattern4.test(data3)) {
                            const err10 = {
                              instancePath: instancePath + "/targetRunId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            };
                            if (vErrors === null) {
                              vErrors = [err10];
                            } else {
                              vErrors.push(err10);
                            }
                            errors++;
                          }
                        }
                      }
                    } else {
                      const err11 = {
                        instancePath: instancePath + "/targetRunId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err11];
                      } else {
                        vErrors.push(err11);
                      }
                      errors++;
                    }
                  }
                  var valid1 = _errs10 === errors;
                } else {
                  var valid1 = true;
                }
              }
            }
          }
        }
      }
    } else {
      const err12 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs13 = errors;
  if (errors === _errs13) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (
        (data.type === undefined && (missing1 = "type")) ||
        (data.targetCommandId === undefined &&
          (missing1 = "targetCommandId")) ||
        (data.generation === undefined && (missing1 = "generation"))
      ) {
        const err13 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing1 },
          message: "must have required property '" + missing1 + "'",
        };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      } else {
        const _errs15 = errors;
        for (const key1 in data) {
          if (
            !(
              key1 === "type" ||
              key1 === "targetCommandId" ||
              key1 === "generation" ||
              key1 === "nativeRunId"
            )
          ) {
            const err14 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key1 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err14];
            } else {
              vErrors.push(err14);
            }
            errors++;
            break;
          }
        }
        if (_errs15 === errors) {
          if (data.type !== undefined) {
            let data4 = data.type;
            const _errs16 = errors;
            if (typeof data4 !== "string") {
              const err15 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err15];
              } else {
                vErrors.push(err15);
              }
              errors++;
            }
            if ("cancel" !== data4) {
              const err16 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/const",
                keyword: "const",
                params: { allowedValue: "cancel" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err16];
              } else {
                vErrors.push(err16);
              }
              errors++;
            }
            var valid3 = _errs16 === errors;
          } else {
            var valid3 = true;
          }
          if (valid3) {
            if (data.targetCommandId !== undefined) {
              let data5 = data.targetCommandId;
              const _errs18 = errors;
              const _errs19 = errors;
              if (errors === _errs19) {
                if (typeof data5 === "string") {
                  if (func1(data5) > 128) {
                    const err17 = {
                      instancePath: instancePath + "/targetCommandId",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err17];
                    } else {
                      vErrors.push(err17);
                    }
                    errors++;
                  } else {
                    if (func1(data5) < 1) {
                      const err18 = {
                        instancePath: instancePath + "/targetCommandId",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err18];
                      } else {
                        vErrors.push(err18);
                      }
                      errors++;
                    } else {
                      if (!pattern4.test(data5)) {
                        const err19 = {
                          instancePath: instancePath + "/targetCommandId",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        };
                        if (vErrors === null) {
                          vErrors = [err19];
                        } else {
                          vErrors.push(err19);
                        }
                        errors++;
                      }
                    }
                  }
                } else {
                  const err20 = {
                    instancePath: instancePath + "/targetCommandId",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err20];
                  } else {
                    vErrors.push(err20);
                  }
                  errors++;
                }
              }
              var valid3 = _errs18 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.generation !== undefined) {
                let data6 = data.generation;
                const _errs21 = errors;
                const _errs22 = errors;
                if (errors === _errs22) {
                  if (typeof data6 === "string") {
                    if (func1(data6) > 128) {
                      const err21 = {
                        instancePath: instancePath + "/generation",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err21];
                      } else {
                        vErrors.push(err21);
                      }
                      errors++;
                    } else {
                      if (func1(data6) < 1) {
                        const err22 = {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err22];
                        } else {
                          vErrors.push(err22);
                        }
                        errors++;
                      } else {
                        if (!pattern4.test(data6)) {
                          const err23 = {
                            instancePath: instancePath + "/generation",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          };
                          if (vErrors === null) {
                            vErrors = [err23];
                          } else {
                            vErrors.push(err23);
                          }
                          errors++;
                        }
                      }
                    }
                  } else {
                    const err24 = {
                      instancePath: instancePath + "/generation",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err24];
                    } else {
                      vErrors.push(err24);
                    }
                    errors++;
                  }
                }
                var valid3 = _errs21 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.nativeRunId !== undefined) {
                  let data7 = data.nativeRunId;
                  const _errs24 = errors;
                  const _errs25 = errors;
                  if (errors === _errs25) {
                    if (typeof data7 === "string") {
                      if (func1(data7) > 128) {
                        const err25 = {
                          instancePath: instancePath + "/nativeRunId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err25];
                        } else {
                          vErrors.push(err25);
                        }
                        errors++;
                      } else {
                        if (func1(data7) < 1) {
                          const err26 = {
                            instancePath: instancePath + "/nativeRunId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err26];
                          } else {
                            vErrors.push(err26);
                          }
                          errors++;
                        } else {
                          if (!pattern4.test(data7)) {
                            const err27 = {
                              instancePath: instancePath + "/nativeRunId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            };
                            if (vErrors === null) {
                              vErrors = [err27];
                            } else {
                              vErrors.push(err27);
                            }
                            errors++;
                          }
                        }
                      }
                    } else {
                      const err28 = {
                        instancePath: instancePath + "/nativeRunId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err28];
                      } else {
                        vErrors.push(err28);
                      }
                      errors++;
                    }
                  }
                  var valid3 = _errs24 === errors;
                } else {
                  var valid3 = true;
                }
              }
            }
          }
        }
      }
    } else {
      const err29 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err29];
      } else {
        vErrors.push(err29);
      }
      errors++;
    }
  }
  var _valid0 = _errs13 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs27 = errors;
    if (errors === _errs27) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing2;
        if (
          (data.type === undefined && (missing2 = "type")) ||
          (data.interactionId === undefined && (missing2 = "interactionId")) ||
          (data.generation === undefined && (missing2 = "generation")) ||
          (data.answer === undefined && (missing2 = "answer"))
        ) {
          const err30 = {
            instancePath,
            schemaPath: "#/oneOf/2/required",
            keyword: "required",
            params: { missingProperty: missing2 },
            message: "must have required property '" + missing2 + "'",
          };
          if (vErrors === null) {
            vErrors = [err30];
          } else {
            vErrors.push(err30);
          }
          errors++;
        } else {
          const _errs29 = errors;
          for (const key2 in data) {
            if (
              !(
                key2 === "type" ||
                key2 === "interactionId" ||
                key2 === "generation" ||
                key2 === "nativeRunId" ||
                key2 === "answer" ||
                key2 === "surface"
              )
            ) {
              const err31 = {
                instancePath,
                schemaPath: "#/oneOf/2/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key2 },
                message: "must NOT have additional properties",
              };
              if (vErrors === null) {
                vErrors = [err31];
              } else {
                vErrors.push(err31);
              }
              errors++;
              break;
            }
          }
          if (_errs29 === errors) {
            if (data.type !== undefined) {
              let data8 = data.type;
              const _errs30 = errors;
              if (typeof data8 !== "string") {
                const err32 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err32];
                } else {
                  vErrors.push(err32);
                }
                errors++;
              }
              if ("respond" !== data8) {
                const err33 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/const",
                  keyword: "const",
                  params: { allowedValue: "respond" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err33];
                } else {
                  vErrors.push(err33);
                }
                errors++;
              }
              var valid7 = _errs30 === errors;
            } else {
              var valid7 = true;
            }
            if (valid7) {
              if (data.interactionId !== undefined) {
                let data9 = data.interactionId;
                const _errs32 = errors;
                const _errs33 = errors;
                if (errors === _errs33) {
                  if (typeof data9 === "string") {
                    if (func1(data9) > 128) {
                      const err34 = {
                        instancePath: instancePath + "/interactionId",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err34];
                      } else {
                        vErrors.push(err34);
                      }
                      errors++;
                    } else {
                      if (func1(data9) < 1) {
                        const err35 = {
                          instancePath: instancePath + "/interactionId",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err35];
                        } else {
                          vErrors.push(err35);
                        }
                        errors++;
                      } else {
                        if (!pattern4.test(data9)) {
                          const err36 = {
                            instancePath: instancePath + "/interactionId",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          };
                          if (vErrors === null) {
                            vErrors = [err36];
                          } else {
                            vErrors.push(err36);
                          }
                          errors++;
                        }
                      }
                    }
                  } else {
                    const err37 = {
                      instancePath: instancePath + "/interactionId",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err37];
                    } else {
                      vErrors.push(err37);
                    }
                    errors++;
                  }
                }
                var valid7 = _errs32 === errors;
              } else {
                var valid7 = true;
              }
              if (valid7) {
                if (data.generation !== undefined) {
                  let data10 = data.generation;
                  const _errs35 = errors;
                  const _errs36 = errors;
                  if (errors === _errs36) {
                    if (typeof data10 === "string") {
                      if (func1(data10) > 128) {
                        const err38 = {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err38];
                        } else {
                          vErrors.push(err38);
                        }
                        errors++;
                      } else {
                        if (func1(data10) < 1) {
                          const err39 = {
                            instancePath: instancePath + "/generation",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err39];
                          } else {
                            vErrors.push(err39);
                          }
                          errors++;
                        } else {
                          if (!pattern4.test(data10)) {
                            const err40 = {
                              instancePath: instancePath + "/generation",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            };
                            if (vErrors === null) {
                              vErrors = [err40];
                            } else {
                              vErrors.push(err40);
                            }
                            errors++;
                          }
                        }
                      }
                    } else {
                      const err41 = {
                        instancePath: instancePath + "/generation",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err41];
                      } else {
                        vErrors.push(err41);
                      }
                      errors++;
                    }
                  }
                  var valid7 = _errs35 === errors;
                } else {
                  var valid7 = true;
                }
                if (valid7) {
                  if (data.nativeRunId !== undefined) {
                    let data11 = data.nativeRunId;
                    const _errs38 = errors;
                    const _errs39 = errors;
                    if (errors === _errs39) {
                      if (typeof data11 === "string") {
                        if (func1(data11) > 128) {
                          const err42 = {
                            instancePath: instancePath + "/nativeRunId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err42];
                          } else {
                            vErrors.push(err42);
                          }
                          errors++;
                        } else {
                          if (func1(data11) < 1) {
                            const err43 = {
                              instancePath: instancePath + "/nativeRunId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            };
                            if (vErrors === null) {
                              vErrors = [err43];
                            } else {
                              vErrors.push(err43);
                            }
                            errors++;
                          } else {
                            if (!pattern4.test(data11)) {
                              const err44 = {
                                instancePath: instancePath + "/nativeRunId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              };
                              if (vErrors === null) {
                                vErrors = [err44];
                              } else {
                                vErrors.push(err44);
                              }
                              errors++;
                            }
                          }
                        }
                      } else {
                        const err45 = {
                          instancePath: instancePath + "/nativeRunId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        };
                        if (vErrors === null) {
                          vErrors = [err45];
                        } else {
                          vErrors.push(err45);
                        }
                        errors++;
                      }
                    }
                    var valid7 = _errs38 === errors;
                  } else {
                    var valid7 = true;
                  }
                  if (valid7) {
                    if (data.answer !== undefined) {
                      let data12 = data.answer;
                      const _errs41 = errors;
                      if (errors === _errs41) {
                        if (
                          data12 &&
                          typeof data12 == "object" &&
                          !Array.isArray(data12)
                        ) {
                        } else {
                          const err46 = {
                            instancePath: instancePath + "/answer",
                            schemaPath: "#/oneOf/2/properties/answer/type",
                            keyword: "type",
                            params: { type: "object" },
                            message: "must be object",
                          };
                          if (vErrors === null) {
                            vErrors = [err46];
                          } else {
                            vErrors.push(err46);
                          }
                          errors++;
                        }
                      }
                      var valid7 = _errs41 === errors;
                    } else {
                      var valid7 = true;
                    }
                    if (valid7) {
                      if (data.surface !== undefined) {
                        const _errs44 = errors;
                        if (
                          !validate23(data.surface, {
                            instancePath: instancePath + "/surface",
                            parentData: data,
                            parentDataProperty: "surface",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate23.errors
                              : vErrors.concat(validate23.errors);
                          errors = vErrors.length;
                        }
                        var valid7 = _errs44 === errors;
                      } else {
                        var valid7 = true;
                      }
                    }
                  }
                }
              }
            }
          }
        }
      } else {
        const err47 = {
          instancePath,
          schemaPath: "#/oneOf/2/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        };
        if (vErrors === null) {
          vErrors = [err47];
        } else {
          vErrors.push(err47);
        }
        errors++;
      }
    }
    var _valid0 = _errs27 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
    }
  }
  if (!valid0) {
    const err48 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err48];
    } else {
      vErrors.push(err48);
    }
    errors++;
    validate22.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate22.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate22.evaluated = { dynamicProps: true, dynamicItems: false };
function validate21(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate21.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.commandId === undefined && (missing0 = "commandId")) ||
        (data.expiresAtMs === undefined && (missing0 = "expiresAtMs")) ||
        (data.input === undefined && (missing0 = "input"))
      ) {
        validate21.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "commandId" ||
              key0 === "expiresAtMs" ||
              key0 === "input"
            )
          ) {
            validate21.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate21.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate21.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("command" !== data1) {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "command" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.commandId !== undefined) {
                  let data3 = data.commandId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/commandId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/commandId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/commandId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/commandId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.expiresAtMs !== undefined) {
                    let data4 = data.expiresAtMs;
                    const _errs12 = errors;
                    const _errs13 = errors;
                    if (
                      !(
                        typeof data4 == "number" &&
                        !(data4 % 1) &&
                        !isNaN(data4) &&
                        isFinite(data4)
                      )
                    ) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/expiresAtMs",
                          schemaPath: "#/$defs/Counter/type",
                          keyword: "type",
                          params: { type: "integer" },
                          message: "must be integer",
                        },
                      ];
                      return false;
                    }
                    if (errors === _errs13) {
                      if (typeof data4 == "number" && isFinite(data4)) {
                        if (data4 > 9007199254740991 || isNaN(data4)) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/expiresAtMs",
                              schemaPath: "#/$defs/Counter/maximum",
                              keyword: "maximum",
                              params: {
                                comparison: "<=",
                                limit: 9007199254740991,
                              },
                              message: "must be <= 9007199254740991",
                            },
                          ];
                          return false;
                        } else {
                          if (data4 < 0 || isNaN(data4)) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/expiresAtMs",
                                schemaPath: "#/$defs/Counter/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.input !== undefined) {
                      const _errs15 = errors;
                      if (
                        !validate22(data.input, {
                          instancePath: instancePath + "/input",
                          parentData: data,
                          parentDataProperty: "input",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate22.errors
                            : vErrors.concat(validate22.errors);
                        errors = vErrors.length;
                      }
                      var valid0 = _errs15 === errors;
                    } else {
                      var valid0 = true;
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate21.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate21.errors = vErrors;
  return errors === 0;
}
validate21.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema47 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "receipt",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    commandId: {
      $ref: "#/$defs/Id",
      description:
        "Client-generated idempotency key; reuse only with identical canonical content.",
    },
    contentHash: {
      type: "string",
      pattern: "^[a-f0-9]{64}$",
      description:
        "SHA-256 of JCS(command), including every command field; namespace is a separate storage key.",
    },
    acceptedAtMs: {
      $ref: "#/$defs/Counter",
      description: "UTC epoch milliseconds at committed acceptance.",
    },
    retryUntilMs: {
      $ref: "#/$defs/Counter",
      description:
        "Inclusive same-command retry deadline, no later than command expiry.",
    },
    receiptUntilMs: {
      $ref: "#/$defs/Counter",
      description:
        "Inclusive stored-receipt deadline; cannot be shorter than the retry deadline.",
    },
    acceptedRevision: {
      $ref: "#/$defs/Counter",
      description: "Session revision which atomically accepted this command.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "commandId",
    "contentHash",
    "acceptedAtMs",
    "retryUntilMs",
    "receiptUntilMs",
    "acceptedRevision",
  ],
  additionalProperties: false,
  description:
    "Immutable acceptance fact. Only an actual committed store makes it durable; it is not a model terminal.",
};
const func22 = Object.prototype.hasOwnProperty;
const schema48 = {
  type: "object",
  properties: {
    tenantId: {
      $ref: "#/$defs/Id",
      description: "Authenticated tenant scope.",
    },
    principalId: {
      $ref: "#/$defs/Id",
      description: "Authenticated principal scope.",
    },
    authorityId: {
      $ref: "#/$defs/Id",
      description: "Authenticated authority scope.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Logical session identifier, never reusable after retirement.",
    },
  },
  required: ["tenantId", "principalId", "authorityId", "sessionId"],
  additionalProperties: false,
  description:
    "Trusted tenant/principal/authority/logical-session storage scope supplied by authenticated ingress.",
};
function validate28(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate28.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.tenantId === undefined && (missing0 = "tenantId")) ||
        (data.principalId === undefined && (missing0 = "principalId")) ||
        (data.authorityId === undefined && (missing0 = "authorityId")) ||
        (data.sessionId === undefined && (missing0 = "sessionId"))
      ) {
        validate28.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "tenantId" ||
              key0 === "principalId" ||
              key0 === "authorityId" ||
              key0 === "sessionId"
            )
          ) {
            validate28.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.tenantId !== undefined) {
            let data0 = data.tenantId;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate28.errors = [
                    {
                      instancePath: instancePath + "/tenantId",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate28.errors = [
                      {
                        instancePath: instancePath + "/tenantId",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate28.errors = [
                        {
                          instancePath: instancePath + "/tenantId",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate28.errors = [
                  {
                    instancePath: instancePath + "/tenantId",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.principalId !== undefined) {
              let data1 = data.principalId;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate28.errors = [
                      {
                        instancePath: instancePath + "/principalId",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate28.errors = [
                        {
                          instancePath: instancePath + "/principalId",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate28.errors = [
                          {
                            instancePath: instancePath + "/principalId",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate28.errors = [
                    {
                      instancePath: instancePath + "/principalId",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.authorityId !== undefined) {
                let data2 = data.authorityId;
                const _errs8 = errors;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate28.errors = [
                        {
                          instancePath: instancePath + "/authorityId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate28.errors = [
                          {
                            instancePath: instancePath + "/authorityId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate28.errors = [
                            {
                              instancePath: instancePath + "/authorityId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate28.errors = [
                      {
                        instancePath: instancePath + "/authorityId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs8 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.sessionId !== undefined) {
                  let data3 = data.sessionId;
                  const _errs11 = errors;
                  const _errs12 = errors;
                  if (errors === _errs12) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate28.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate28.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate28.errors = [
                              {
                                instancePath: instancePath + "/sessionId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate28.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs11 === errors;
                } else {
                  var valid0 = true;
                }
              }
            }
          }
        }
      }
    } else {
      validate28.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate28.errors = vErrors;
  return errors === 0;
}
validate28.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const pattern19 = new RegExp("^[a-f0-9]{64}$", "u");
function validate27(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate27.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.commandId === undefined && (missing0 = "commandId")) ||
        (data.contentHash === undefined && (missing0 = "contentHash")) ||
        (data.acceptedAtMs === undefined && (missing0 = "acceptedAtMs")) ||
        (data.retryUntilMs === undefined && (missing0 = "retryUntilMs")) ||
        (data.receiptUntilMs === undefined && (missing0 = "receiptUntilMs")) ||
        (data.acceptedRevision === undefined && (missing0 = "acceptedRevision"))
      ) {
        validate27.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema47.properties, key0)) {
            validate27.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate27.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate27.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate27.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("receipt" !== data1) {
                validate27.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "receipt" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.commandId !== undefined) {
                  let data3 = data.commandId;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate27.errors = [
                          {
                            instancePath: instancePath + "/commandId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate27.errors = [
                            {
                              instancePath: instancePath + "/commandId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate27.errors = [
                              {
                                instancePath: instancePath + "/commandId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate27.errors = [
                        {
                          instancePath: instancePath + "/commandId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.contentHash !== undefined) {
                    let data4 = data.contentHash;
                    const _errs10 = errors;
                    if (errors === _errs10) {
                      if (typeof data4 === "string") {
                        if (!pattern19.test(data4)) {
                          validate27.errors = [
                            {
                              instancePath: instancePath + "/contentHash",
                              schemaPath: "#/properties/contentHash/pattern",
                              keyword: "pattern",
                              params: { pattern: "^[a-f0-9]{64}$" },
                              message:
                                'must match pattern "' + "^[a-f0-9]{64}$" + '"',
                            },
                          ];
                          return false;
                        }
                      } else {
                        validate27.errors = [
                          {
                            instancePath: instancePath + "/contentHash",
                            schemaPath: "#/properties/contentHash/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.acceptedAtMs !== undefined) {
                      let data5 = data.acceptedAtMs;
                      const _errs12 = errors;
                      const _errs13 = errors;
                      if (
                        !(
                          typeof data5 == "number" &&
                          !(data5 % 1) &&
                          !isNaN(data5) &&
                          isFinite(data5)
                        )
                      ) {
                        validate27.errors = [
                          {
                            instancePath: instancePath + "/acceptedAtMs",
                            schemaPath: "#/$defs/Counter/type",
                            keyword: "type",
                            params: { type: "integer" },
                            message: "must be integer",
                          },
                        ];
                        return false;
                      }
                      if (errors === _errs13) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate27.errors = [
                              {
                                instancePath: instancePath + "/acceptedAtMs",
                                schemaPath: "#/$defs/Counter/maximum",
                                keyword: "maximum",
                                params: {
                                  comparison: "<=",
                                  limit: 9007199254740991,
                                },
                                message: "must be <= 9007199254740991",
                              },
                            ];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate27.errors = [
                                {
                                  instancePath: instancePath + "/acceptedAtMs",
                                  schemaPath: "#/$defs/Counter/minimum",
                                  keyword: "minimum",
                                  params: { comparison: ">=", limit: 0 },
                                  message: "must be >= 0",
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      }
                      var valid0 = _errs12 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.retryUntilMs !== undefined) {
                        let data6 = data.retryUntilMs;
                        const _errs15 = errors;
                        const _errs16 = errors;
                        if (
                          !(
                            typeof data6 == "number" &&
                            !(data6 % 1) &&
                            !isNaN(data6) &&
                            isFinite(data6)
                          )
                        ) {
                          validate27.errors = [
                            {
                              instancePath: instancePath + "/retryUntilMs",
                              schemaPath: "#/$defs/Counter/type",
                              keyword: "type",
                              params: { type: "integer" },
                              message: "must be integer",
                            },
                          ];
                          return false;
                        }
                        if (errors === _errs16) {
                          if (typeof data6 == "number" && isFinite(data6)) {
                            if (data6 > 9007199254740991 || isNaN(data6)) {
                              validate27.errors = [
                                {
                                  instancePath: instancePath + "/retryUntilMs",
                                  schemaPath: "#/$defs/Counter/maximum",
                                  keyword: "maximum",
                                  params: {
                                    comparison: "<=",
                                    limit: 9007199254740991,
                                  },
                                  message: "must be <= 9007199254740991",
                                },
                              ];
                              return false;
                            } else {
                              if (data6 < 0 || isNaN(data6)) {
                                validate27.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/retryUntilMs",
                                    schemaPath: "#/$defs/Counter/minimum",
                                    keyword: "minimum",
                                    params: { comparison: ">=", limit: 0 },
                                    message: "must be >= 0",
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        }
                        var valid0 = _errs15 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.receiptUntilMs !== undefined) {
                          let data7 = data.receiptUntilMs;
                          const _errs18 = errors;
                          const _errs19 = errors;
                          if (
                            !(
                              typeof data7 == "number" &&
                              !(data7 % 1) &&
                              !isNaN(data7) &&
                              isFinite(data7)
                            )
                          ) {
                            validate27.errors = [
                              {
                                instancePath: instancePath + "/receiptUntilMs",
                                schemaPath: "#/$defs/Counter/type",
                                keyword: "type",
                                params: { type: "integer" },
                                message: "must be integer",
                              },
                            ];
                            return false;
                          }
                          if (errors === _errs19) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 9007199254740991 || isNaN(data7)) {
                                validate27.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/receiptUntilMs",
                                    schemaPath: "#/$defs/Counter/maximum",
                                    keyword: "maximum",
                                    params: {
                                      comparison: "<=",
                                      limit: 9007199254740991,
                                    },
                                    message: "must be <= 9007199254740991",
                                  },
                                ];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate27.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/receiptUntilMs",
                                      schemaPath: "#/$defs/Counter/minimum",
                                      keyword: "minimum",
                                      params: { comparison: ">=", limit: 0 },
                                      message: "must be >= 0",
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs18 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.acceptedRevision !== undefined) {
                            let data8 = data.acceptedRevision;
                            const _errs21 = errors;
                            const _errs22 = errors;
                            if (
                              !(
                                typeof data8 == "number" &&
                                !(data8 % 1) &&
                                !isNaN(data8) &&
                                isFinite(data8)
                              )
                            ) {
                              validate27.errors = [
                                {
                                  instancePath:
                                    instancePath + "/acceptedRevision",
                                  schemaPath: "#/$defs/Counter/type",
                                  keyword: "type",
                                  params: { type: "integer" },
                                  message: "must be integer",
                                },
                              ];
                              return false;
                            }
                            if (errors === _errs22) {
                              if (typeof data8 == "number" && isFinite(data8)) {
                                if (data8 > 9007199254740991 || isNaN(data8)) {
                                  validate27.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/acceptedRevision",
                                      schemaPath: "#/$defs/Counter/maximum",
                                      keyword: "maximum",
                                      params: {
                                        comparison: "<=",
                                        limit: 9007199254740991,
                                      },
                                      message: "must be <= 9007199254740991",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (data8 < 0 || isNaN(data8)) {
                                    validate27.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/acceptedRevision",
                                        schemaPath: "#/$defs/Counter/minimum",
                                        keyword: "minimum",
                                        params: { comparison: ">=", limit: 0 },
                                        message: "must be >= 0",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                              }
                            }
                            var valid0 = _errs21 === errors;
                          } else {
                            var valid0 = true;
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate27.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate27.errors = vErrors;
  return errors === 0;
}
validate27.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema58 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "commandRecord",
      description: "Closed product record discriminator.",
    },
    command: {
      $ref: "#/$defs/Command",
      description: "Immutable original command.",
    },
    receipt: {
      $ref: "#/$defs/Receipt",
      description: "Immutable original committed acceptance fact.",
    },
    state: {
      $ref: "#/$defs/CommandState",
      description:
        "Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence.",
    },
    dispatch: {
      $ref: "#/$defs/Dispatch",
      description:
        "Native correlation persisted before/with dispatch; never reconstructed from UI history.",
    },
    outcome: {
      $ref: "#/$defs/Outcome",
      description:
        "Definite model-turn result; no implication about business side effects.",
    },
    failure: {
      $ref: "#/$defs/Failure",
      description: "Closed failure category and retry discipline.",
    },
  },
  required: ["schemaVersion", "kind", "command", "receipt", "state"],
  additionalProperties: false,
  description:
    "Single inbox/dispatch ledger; no second provider queue owns the same command.",
};
const schema59 = {
  type: "string",
  enum: [
    "accepted",
    "dispatching",
    "running",
    "terminal",
    "reconciliation_required",
  ],
  description:
    "accepted persists intent; dispatching persists dispatch intent; running has native confirmation; terminal has a definite outcome; reconciliation_required forbids blind resubmission.",
};
const schema65 = {
  type: "string",
  enum: [
    "completed",
    "cancelled",
    "refused",
    "max_tokens",
    "max_turn_requests",
    "failed",
  ],
  description:
    "Definite model-turn outcome; does not establish process exit or business-side-effect completion.",
};
const schema60 = {
  type: "object",
  properties: {
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    nativeSessionId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned context session identifier; history alone cannot recreate it.",
    },
    nativeRunId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned model-turn/run identifier, required when the provider exposes it.",
    },
    nativeRequestId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction.",
    },
    certainty: {
      type: "string",
      enum: ["not_sent", "submitted", "unknown"],
      description:
        "submitted has native acceptance; unknown requires reconciliation; not_sent has evidence no submission occurred.",
    },
  },
  required: ["generation", "nativeSessionId", "certainty"],
  additionalProperties: false,
  description:
    "Persisted native correlation and certainty; unknown requires reconciliation before any further send.",
};
function validate34(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate34.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.nativeSessionId === undefined &&
          (missing0 = "nativeSessionId")) ||
        (data.certainty === undefined && (missing0 = "certainty"))
      ) {
        validate34.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "generation" ||
              key0 === "nativeSessionId" ||
              key0 === "nativeRunId" ||
              key0 === "nativeRequestId" ||
              key0 === "certainty"
            )
          ) {
            validate34.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.generation !== undefined) {
            let data0 = data.generation;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate34.errors = [
                    {
                      instancePath: instancePath + "/generation",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate34.errors = [
                      {
                        instancePath: instancePath + "/generation",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate34.errors = [
                  {
                    instancePath: instancePath + "/generation",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.nativeSessionId !== undefined) {
              let data1 = data.nativeSessionId;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate34.errors = [
                      {
                        instancePath: instancePath + "/nativeSessionId",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/nativeSessionId",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate34.errors = [
                          {
                            instancePath: instancePath + "/nativeSessionId",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate34.errors = [
                    {
                      instancePath: instancePath + "/nativeSessionId",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.nativeRunId !== undefined) {
                let data2 = data.nativeRunId;
                const _errs8 = errors;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/nativeRunId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate34.errors = [
                          {
                            instancePath: instancePath + "/nativeRunId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate34.errors = [
                            {
                              instancePath: instancePath + "/nativeRunId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate34.errors = [
                      {
                        instancePath: instancePath + "/nativeRunId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs8 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.nativeRequestId !== undefined) {
                  let data3 = data.nativeRequestId;
                  const _errs11 = errors;
                  const _errs12 = errors;
                  if (errors === _errs12) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate34.errors = [
                          {
                            instancePath: instancePath + "/nativeRequestId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate34.errors = [
                            {
                              instancePath: instancePath + "/nativeRequestId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate34.errors = [
                              {
                                instancePath: instancePath + "/nativeRequestId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/nativeRequestId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs11 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.certainty !== undefined) {
                    let data4 = data.certainty;
                    const _errs14 = errors;
                    if (typeof data4 !== "string") {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/certainty",
                          schemaPath: "#/properties/certainty/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "not_sent" ||
                        data4 === "submitted" ||
                        data4 === "unknown"
                      )
                    ) {
                      validate34.errors = [
                        {
                          instancePath: instancePath + "/certainty",
                          schemaPath: "#/properties/certainty/enum",
                          keyword: "enum",
                          params: {
                            allowedValues: schema60.properties.certainty.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid0 = _errs14 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate34.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate34.errors = vErrors;
  return errors === 0;
}
validate34.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema66 = {
  type: "object",
  properties: {
    code: {
      $ref: "#/$defs/ErrorCode",
      description: "Closed diagnostic category without input values.",
    },
    retry: {
      $ref: "#/$defs/Retry",
      description:
        "Explicit retry discipline; uncertainty never authorizes blind resubmission.",
    },
  },
  required: ["code", "retry"],
  additionalProperties: false,
  description: "Value-free failure and explicit retry discipline.",
};
const schema67 = {
  type: "string",
  enum: [
    "invalid_input",
    "unsupported_version",
    "unsupported_capability",
    "permission_denied",
    "content_conflict",
    "revision_conflict",
    "stale_binding",
    "expired",
    "unavailable",
    "reconciliation_required",
    "limit_exceeded",
    "cursor_expired",
    "session_gone",
    "already_answered",
  ],
  description:
    "Closed value-free error category; diagnostics never include model text or credentials.",
};
const schema68 = {
  type: "string",
  enum: ["same_command", "reconcile_first", "never"],
  description:
    "same_command preserves identity/content; reconcile_first checks the original operation; never forbids retry.",
};
function validate36(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate36.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.code === undefined && (missing0 = "code")) ||
        (data.retry === undefined && (missing0 = "retry"))
      ) {
        validate36.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "code" || key0 === "retry")) {
            validate36.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.code !== undefined) {
            let data0 = data.code;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate36.errors = [
                {
                  instancePath: instancePath + "/code",
                  schemaPath: "#/$defs/ErrorCode/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                },
              ];
              return false;
            }
            if (
              !(
                data0 === "invalid_input" ||
                data0 === "unsupported_version" ||
                data0 === "unsupported_capability" ||
                data0 === "permission_denied" ||
                data0 === "content_conflict" ||
                data0 === "revision_conflict" ||
                data0 === "stale_binding" ||
                data0 === "expired" ||
                data0 === "unavailable" ||
                data0 === "reconciliation_required" ||
                data0 === "limit_exceeded" ||
                data0 === "cursor_expired" ||
                data0 === "session_gone" ||
                data0 === "already_answered"
              )
            ) {
              validate36.errors = [
                {
                  instancePath: instancePath + "/code",
                  schemaPath: "#/$defs/ErrorCode/enum",
                  keyword: "enum",
                  params: { allowedValues: schema67.enum },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.retry !== undefined) {
              let data1 = data.retry;
              const _errs5 = errors;
              if (typeof data1 !== "string") {
                validate36.errors = [
                  {
                    instancePath: instancePath + "/retry",
                    schemaPath: "#/$defs/Retry/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if (
                !(
                  data1 === "same_command" ||
                  data1 === "reconcile_first" ||
                  data1 === "never"
                )
              ) {
                validate36.errors = [
                  {
                    instancePath: instancePath + "/retry",
                    schemaPath: "#/$defs/Retry/enum",
                    keyword: "enum",
                    params: { allowedValues: schema68.enum },
                    message: "must be equal to one of the allowed values",
                  },
                ];
                return false;
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate36.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate36.errors = vErrors;
  return errors === 0;
}
validate36.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate31(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate31.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.command === undefined && (missing0 = "command")) ||
        (data.receipt === undefined && (missing0 = "receipt")) ||
        (data.state === undefined && (missing0 = "state"))
      ) {
        validate31.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "command" ||
              key0 === "receipt" ||
              key0 === "state" ||
              key0 === "dispatch" ||
              key0 === "outcome" ||
              key0 === "failure"
            )
          ) {
            validate31.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate31.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate31.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate31.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("commandRecord" !== data1) {
                validate31.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "commandRecord" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.command !== undefined) {
                const _errs6 = errors;
                if (
                  !validate21(data.command, {
                    instancePath: instancePath + "/command",
                    parentData: data,
                    parentDataProperty: "command",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate21.errors
                      : vErrors.concat(validate21.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.receipt !== undefined) {
                  const _errs7 = errors;
                  if (
                    !validate27(data.receipt, {
                      instancePath: instancePath + "/receipt",
                      parentData: data,
                      parentDataProperty: "receipt",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate27.errors
                        : vErrors.concat(validate27.errors);
                    errors = vErrors.length;
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.state !== undefined) {
                    let data4 = data.state;
                    const _errs8 = errors;
                    if (typeof data4 !== "string") {
                      validate31.errors = [
                        {
                          instancePath: instancePath + "/state",
                          schemaPath: "#/$defs/CommandState/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "accepted" ||
                        data4 === "dispatching" ||
                        data4 === "running" ||
                        data4 === "terminal" ||
                        data4 === "reconciliation_required"
                      )
                    ) {
                      validate31.errors = [
                        {
                          instancePath: instancePath + "/state",
                          schemaPath: "#/$defs/CommandState/enum",
                          keyword: "enum",
                          params: { allowedValues: schema59.enum },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid0 = _errs8 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.dispatch !== undefined) {
                      const _errs11 = errors;
                      if (
                        !validate34(data.dispatch, {
                          instancePath: instancePath + "/dispatch",
                          parentData: data,
                          parentDataProperty: "dispatch",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate34.errors
                            : vErrors.concat(validate34.errors);
                        errors = vErrors.length;
                      }
                      var valid0 = _errs11 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.outcome !== undefined) {
                        let data6 = data.outcome;
                        const _errs12 = errors;
                        if (typeof data6 !== "string") {
                          validate31.errors = [
                            {
                              instancePath: instancePath + "/outcome",
                              schemaPath: "#/$defs/Outcome/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        if (
                          !(
                            data6 === "completed" ||
                            data6 === "cancelled" ||
                            data6 === "refused" ||
                            data6 === "max_tokens" ||
                            data6 === "max_turn_requests" ||
                            data6 === "failed"
                          )
                        ) {
                          validate31.errors = [
                            {
                              instancePath: instancePath + "/outcome",
                              schemaPath: "#/$defs/Outcome/enum",
                              keyword: "enum",
                              params: { allowedValues: schema65.enum },
                              message:
                                "must be equal to one of the allowed values",
                            },
                          ];
                          return false;
                        }
                        var valid0 = _errs12 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.failure !== undefined) {
                          const _errs15 = errors;
                          if (
                            !validate36(data.failure, {
                              instancePath: instancePath + "/failure",
                              parentData: data,
                              parentDataProperty: "failure",
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate36.errors
                                : vErrors.concat(validate36.errors);
                            errors = vErrors.length;
                          }
                          var valid0 = _errs15 === errors;
                        } else {
                          var valid0 = true;
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate31.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate31.errors = vErrors;
  return errors === 0;
}
validate31.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema69 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "event",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    eventId: {
      $ref: "#/$defs/Id",
      description: "Stable unique event identifier within the namespace.",
    },
    sequence: {
      $ref: "#/$defs/Counter",
      description:
        "Strictly increasing stable-event counter; attach cursors are exclusive.",
    },
    commandId: {
      $ref: "#/$defs/Id",
      description:
        "Client-generated idempotency key; reuse only with identical canonical content.",
    },
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    body: {
      $ref: "#/$defs/EventBody",
      description: "Stable observation committed before publication.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "eventId",
    "sequence",
    "commandId",
    "generation",
    "body",
  ],
  additionalProperties: false,
  description:
    "Committed stable event in one namespace; token deltas are excluded from durable ordering.",
};
const schema74 = {
  oneOf: [
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "text",
          description: "Closed variant discriminator.",
        },
        messageId: {
          $ref: "#/$defs/Id",
          description: "Stable product message correlation identifier.",
        },
        text: {
          type: "string",
          maxLength: 65536,
          description:
            "Untrusted model/user text subject to the whole-envelope budgets.",
        },
      },
      required: ["type", "messageId", "text"],
      additionalProperties: false,
      description:
        "text variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "status",
          description: "Closed variant discriminator.",
        },
        state: {
          $ref: "#/$defs/CommandState",
          description:
            "Explicit command lifecycle state; terminal and reconciliation transitions require matching evidence.",
        },
      },
      required: ["type", "state"],
      additionalProperties: false,
      description:
        "status variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "terminal",
          description: "Closed variant discriminator.",
        },
        outcome: {
          $ref: "#/$defs/Outcome",
          description:
            "Definite model-turn result; no implication about business side effects.",
        },
      },
      required: ["type", "outcome"],
      additionalProperties: false,
      description:
        "terminal variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "cancel_dispatched",
          description: "Closed variant discriminator.",
        },
        confirmation: {
          type: "string",
          enum: ["request_only", "already_terminal", "unsupported"],
          description:
            "Cancellation request transport confirmation only; does not manufacture a model terminal.",
        },
      },
      required: ["type", "confirmation"],
      additionalProperties: false,
      description:
        "cancel_dispatched variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "tool_proposal",
          description: "Closed variant discriminator.",
        },
        proposalId: {
          $ref: "#/$defs/Id",
          description: "Untrusted tool proposal correlation identifier.",
        },
        name: {
          $ref: "#/$defs/Id",
          description: "Provider tool name; not an approved execution action.",
        },
        arguments: {
          type: "object",
          additionalProperties: true,
          description:
            "Untrusted tool JSON arguments, including keys, count toward product budgets.",
        },
      },
      required: ["type", "proposalId", "name", "arguments"],
      additionalProperties: false,
      description:
        "tool_proposal variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "tool_result",
          description: "Closed variant discriminator.",
        },
        proposalId: {
          $ref: "#/$defs/Id",
          description: "Untrusted tool proposal correlation identifier.",
        },
        disposition: {
          type: "string",
          enum: ["returned", "rejected", "unavailable"],
          description:
            "Protocol tool-result disposition, not authoritative business execution status.",
        },
        text: {
          type: "string",
          maxLength: 65536,
          description:
            "Untrusted model/user text subject to the whole-envelope budgets.",
        },
      },
      required: ["type", "proposalId", "disposition", "text"],
      additionalProperties: false,
      description:
        "tool_result variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "interaction",
          description: "Closed variant discriminator.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        status: {
          type: "string",
          const: "pending",
          description: "First publication of an ordinary user question.",
        },
        request: {
          $ref: "#/$defs/InteractionRequest",
          description:
            "Required for the first pending event and equal to the newly committed Interaction request; forbidden on later lifecycle events.",
        },
        expiresAtMs: {
          $ref: "#/$defs/Counter",
          description:
            "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
        },
        callbackLifetime: {
          $ref: "#/$defs/CallbackLifetime",
          description:
            "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
        },
      },
      required: [
        "type",
        "interactionId",
        "status",
        "request",
        "expiresAtMs",
        "callbackLifetime",
      ],
      additionalProperties: false,
      description:
        "Initial ordinary question publication; the matching Interaction is committed atomically.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "interaction",
          description: "Closed variant discriminator.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        status: {
          type: "string",
          const: "answered",
          description: "The first accepted response consumed this interaction.",
        },
        responseCommandId: {
          $ref: "#/$defs/Id",
          description:
            "Accepted response command which atomically consumed the interaction; present only when answered.",
        },
      },
      required: ["type", "interactionId", "status", "responseCommandId"],
      additionalProperties: false,
      description:
        "First accepted response identity, committed atomically with the receipt and Interaction.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "interaction",
          description: "Closed variant discriminator.",
        },
        interactionId: {
          $ref: "#/$defs/Id",
          description: "Single-use interaction identity within the namespace.",
        },
        status: {
          type: "string",
          enum: ["expired", "unavailable"],
          description:
            "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
        },
      },
      required: ["type", "interactionId", "status"],
      additionalProperties: false,
      description:
        "Question lifecycle transition; cannot republish or replace its request.",
    },
    {
      type: "object",
      properties: {
        type: {
          type: "string",
          const: "error",
          description: "Closed variant discriminator.",
        },
        failure: {
          $ref: "#/$defs/Failure",
          description: "Closed failure category and retry discipline.",
        },
      },
      required: ["type", "failure"],
      additionalProperties: false,
      description:
        "error variant; all fields are data, never authentication or execution authority.",
    },
    {
      type: "object",
      properties: {
        type: {
          const: "surface",
          type: "string",
          description: "Closed variant discriminator.",
        },
        surface: {
          $ref: "#/$defs/SurfaceState",
          description: "Full surface recovery state committed with this event.",
        },
      },
      required: ["type", "surface"],
      additionalProperties: false,
    },
  ],
  description:
    "Stable product observation. Tool proposals and results are untrusted and cannot issue execution authority.",
};
const schema82 = {
  type: "object",
  additionalProperties: true,
  description:
    "Immutable untrusted provider question payload. Subject to whole-record JSON budgets; never authentication, permission or execution approval.",
};
const schema84 = {
  type: "string",
  enum: ["generation_bound", "provider_resumable"],
  description:
    "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
};
const schema88 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "surface",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    nativeRunId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned model-turn/run identifier, required when the provider exposes it.",
    },
    surfaceId: {
      $ref: "#/$defs/Id",
      description: "Upstream A2UI surface identifier.",
    },
    surfaceInstanceId: {
      $ref: "#/$defs/Id",
      description:
        "Fresh product identity for each surface creation; deletion permanently invalidates old actions.",
    },
    revision: {
      $ref: "#/$defs/Counter",
      description: "Monotonic CAS revision of this product record.",
    },
    interactionId: {
      $ref: "#/$defs/Id",
      description: "Single-use interaction identity within the namespace.",
    },
    sourceComponentId: {
      $ref: "#/$defs/Id",
      description:
        "Exact upstream source component allowed to emit this action.",
    },
    eventName: {
      $ref: "#/$defs/Id",
      description:
        "Exact upstream action name associated with this interaction.",
    },
    catalogId: {
      $ref: "#/$defs/Id",
      description: "Negotiated upstream catalog identity.",
    },
    catalogVersion: {
      $ref: "#/$defs/Id",
      description:
        "Negotiated fixed catalog version; not a renderer implementation claim.",
    },
    a2uiVersion: {
      type: "string",
      const: "v0.9.1",
      description: "Exact negotiated upstream A2UI version.",
    },
    status: {
      type: "string",
      enum: ["active", "deleted"],
      description:
        "Persisted lifecycle; deleted is a permanent tombstone for this instance.",
    },
    messages: {
      type: "array",
      maxItems: 128,
      items: { type: "object", additionalProperties: true },
      description:
        "Bounded unchanged upstream messages needed to rebuild this instance; not a second A2UI schema.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "generation",
    "nativeRunId",
    "surfaceId",
    "surfaceInstanceId",
    "revision",
    "interactionId",
    "sourceComponentId",
    "eventName",
    "catalogId",
    "catalogVersion",
    "a2uiVersion",
    "status",
    "messages",
  ],
  additionalProperties: false,
  description:
    "Single surface record: association, lifecycle and bounded upstream recovery content.",
};
function validate43(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate43.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.nativeRunId === undefined && (missing0 = "nativeRunId")) ||
        (data.surfaceId === undefined && (missing0 = "surfaceId")) ||
        (data.surfaceInstanceId === undefined &&
          (missing0 = "surfaceInstanceId")) ||
        (data.revision === undefined && (missing0 = "revision")) ||
        (data.interactionId === undefined && (missing0 = "interactionId")) ||
        (data.sourceComponentId === undefined &&
          (missing0 = "sourceComponentId")) ||
        (data.eventName === undefined && (missing0 = "eventName")) ||
        (data.catalogId === undefined && (missing0 = "catalogId")) ||
        (data.catalogVersion === undefined && (missing0 = "catalogVersion")) ||
        (data.a2uiVersion === undefined && (missing0 = "a2uiVersion")) ||
        (data.status === undefined && (missing0 = "status")) ||
        (data.messages === undefined && (missing0 = "messages"))
      ) {
        validate43.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema88.properties, key0)) {
            validate43.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate43.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate43.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate43.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("surface" !== data1) {
                validate43.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "surface" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.generation !== undefined) {
                  let data3 = data.generation;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate43.errors = [
                          {
                            instancePath: instancePath + "/generation",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate43.errors = [
                            {
                              instancePath: instancePath + "/generation",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate43.errors = [
                              {
                                instancePath: instancePath + "/generation",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate43.errors = [
                        {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.nativeRunId !== undefined) {
                    let data4 = data.nativeRunId;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (errors === _errs11) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate43.errors = [
                            {
                              instancePath: instancePath + "/nativeRunId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate43.errors = [
                              {
                                instancePath: instancePath + "/nativeRunId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate43.errors = [
                                {
                                  instancePath: instancePath + "/nativeRunId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate43.errors = [
                          {
                            instancePath: instancePath + "/nativeRunId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.surfaceId !== undefined) {
                      let data5 = data.surfaceId;
                      const _errs13 = errors;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (typeof data5 === "string") {
                          if (func1(data5) > 128) {
                            validate43.errors = [
                              {
                                instancePath: instancePath + "/surfaceId",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (func1(data5) < 1) {
                              validate43.errors = [
                                {
                                  instancePath: instancePath + "/surfaceId",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (!pattern4.test(data5)) {
                                validate43.errors = [
                                  {
                                    instancePath: instancePath + "/surfaceId",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate43.errors = [
                            {
                              instancePath: instancePath + "/surfaceId",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.surfaceInstanceId !== undefined) {
                        let data6 = data.surfaceInstanceId;
                        const _errs16 = errors;
                        const _errs17 = errors;
                        if (errors === _errs17) {
                          if (typeof data6 === "string") {
                            if (func1(data6) > 128) {
                              validate43.errors = [
                                {
                                  instancePath:
                                    instancePath + "/surfaceInstanceId",
                                  schemaPath: "#/$defs/Id/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (func1(data6) < 1) {
                                validate43.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/surfaceInstanceId",
                                    schemaPath: "#/$defs/Id/minLength",
                                    keyword: "minLength",
                                    params: { limit: 1 },
                                    message:
                                      "must NOT have fewer than 1 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (!pattern4.test(data6)) {
                                  validate43.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/surfaceInstanceId",
                                      schemaPath: "#/$defs/Id/pattern",
                                      keyword: "pattern",
                                      params: {
                                        pattern:
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                      },
                                      message:
                                        'must match pattern "' +
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                        '"',
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          } else {
                            validate43.errors = [
                              {
                                instancePath:
                                  instancePath + "/surfaceInstanceId",
                                schemaPath: "#/$defs/Id/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs16 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.revision !== undefined) {
                          let data7 = data.revision;
                          const _errs19 = errors;
                          const _errs20 = errors;
                          if (
                            !(
                              typeof data7 == "number" &&
                              !(data7 % 1) &&
                              !isNaN(data7) &&
                              isFinite(data7)
                            )
                          ) {
                            validate43.errors = [
                              {
                                instancePath: instancePath + "/revision",
                                schemaPath: "#/$defs/Counter/type",
                                keyword: "type",
                                params: { type: "integer" },
                                message: "must be integer",
                              },
                            ];
                            return false;
                          }
                          if (errors === _errs20) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 9007199254740991 || isNaN(data7)) {
                                validate43.errors = [
                                  {
                                    instancePath: instancePath + "/revision",
                                    schemaPath: "#/$defs/Counter/maximum",
                                    keyword: "maximum",
                                    params: {
                                      comparison: "<=",
                                      limit: 9007199254740991,
                                    },
                                    message: "must be <= 9007199254740991",
                                  },
                                ];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate43.errors = [
                                    {
                                      instancePath: instancePath + "/revision",
                                      schemaPath: "#/$defs/Counter/minimum",
                                      keyword: "minimum",
                                      params: { comparison: ">=", limit: 0 },
                                      message: "must be >= 0",
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs19 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.interactionId !== undefined) {
                            let data8 = data.interactionId;
                            const _errs22 = errors;
                            const _errs23 = errors;
                            if (errors === _errs23) {
                              if (typeof data8 === "string") {
                                if (func1(data8) > 128) {
                                  validate43.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/interactionId",
                                      schemaPath: "#/$defs/Id/maxLength",
                                      keyword: "maxLength",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (func1(data8) < 1) {
                                    validate43.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/interactionId",
                                        schemaPath: "#/$defs/Id/minLength",
                                        keyword: "minLength",
                                        params: { limit: 1 },
                                        message:
                                          "must NOT have fewer than 1 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (!pattern4.test(data8)) {
                                      validate43.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/interactionId",
                                          schemaPath: "#/$defs/Id/pattern",
                                          keyword: "pattern",
                                          params: {
                                            pattern:
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                          },
                                          message:
                                            'must match pattern "' +
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                            '"',
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate43.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath: "#/$defs/Id/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs22 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.sourceComponentId !== undefined) {
                              let data9 = data.sourceComponentId;
                              const _errs25 = errors;
                              const _errs26 = errors;
                              if (errors === _errs26) {
                                if (typeof data9 === "string") {
                                  if (func1(data9) > 128) {
                                    validate43.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/sourceComponentId",
                                        schemaPath: "#/$defs/Id/maxLength",
                                        keyword: "maxLength",
                                        params: { limit: 128 },
                                        message:
                                          "must NOT have more than 128 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (func1(data9) < 1) {
                                      validate43.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/sourceComponentId",
                                          schemaPath: "#/$defs/Id/minLength",
                                          keyword: "minLength",
                                          params: { limit: 1 },
                                          message:
                                            "must NOT have fewer than 1 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (!pattern4.test(data9)) {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/sourceComponentId",
                                            schemaPath: "#/$defs/Id/pattern",
                                            keyword: "pattern",
                                            params: {
                                              pattern:
                                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                            },
                                            message:
                                              'must match pattern "' +
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                              '"',
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate43.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/sourceComponentId",
                                      schemaPath: "#/$defs/Id/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid0 = _errs25 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.eventName !== undefined) {
                                let data10 = data.eventName;
                                const _errs28 = errors;
                                const _errs29 = errors;
                                if (errors === _errs29) {
                                  if (typeof data10 === "string") {
                                    if (func1(data10) > 128) {
                                      validate43.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/eventName",
                                          schemaPath: "#/$defs/Id/maxLength",
                                          keyword: "maxLength",
                                          params: { limit: 128 },
                                          message:
                                            "must NOT have more than 128 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (func1(data10) < 1) {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/eventName",
                                            schemaPath: "#/$defs/Id/minLength",
                                            keyword: "minLength",
                                            params: { limit: 1 },
                                            message:
                                              "must NOT have fewer than 1 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (!pattern4.test(data10)) {
                                          validate43.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/eventName",
                                              schemaPath: "#/$defs/Id/pattern",
                                              keyword: "pattern",
                                              params: {
                                                pattern:
                                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                              },
                                              message:
                                                'must match pattern "' +
                                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                                '"',
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                    }
                                  } else {
                                    validate43.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/eventName",
                                        schemaPath: "#/$defs/Id/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid0 = _errs28 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.catalogId !== undefined) {
                                  let data11 = data.catalogId;
                                  const _errs31 = errors;
                                  const _errs32 = errors;
                                  if (errors === _errs32) {
                                    if (typeof data11 === "string") {
                                      if (func1(data11) > 128) {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/catalogId",
                                            schemaPath: "#/$defs/Id/maxLength",
                                            keyword: "maxLength",
                                            params: { limit: 128 },
                                            message:
                                              "must NOT have more than 128 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (func1(data11) < 1) {
                                          validate43.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/catalogId",
                                              schemaPath:
                                                "#/$defs/Id/minLength",
                                              keyword: "minLength",
                                              params: { limit: 1 },
                                              message:
                                                "must NOT have fewer than 1 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (!pattern4.test(data11)) {
                                            validate43.errors = [
                                              {
                                                instancePath:
                                                  instancePath + "/catalogId",
                                                schemaPath:
                                                  "#/$defs/Id/pattern",
                                                keyword: "pattern",
                                                params: {
                                                  pattern:
                                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                                },
                                                message:
                                                  'must match pattern "' +
                                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                                  '"',
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                      }
                                    } else {
                                      validate43.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/catalogId",
                                          schemaPath: "#/$defs/Id/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid0 = _errs31 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.catalogVersion !== undefined) {
                                    let data12 = data.catalogVersion;
                                    const _errs34 = errors;
                                    const _errs35 = errors;
                                    if (errors === _errs35) {
                                      if (typeof data12 === "string") {
                                        if (func1(data12) > 128) {
                                          validate43.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/catalogVersion",
                                              schemaPath:
                                                "#/$defs/Id/maxLength",
                                              keyword: "maxLength",
                                              params: { limit: 128 },
                                              message:
                                                "must NOT have more than 128 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (func1(data12) < 1) {
                                            validate43.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/catalogVersion",
                                                schemaPath:
                                                  "#/$defs/Id/minLength",
                                                keyword: "minLength",
                                                params: { limit: 1 },
                                                message:
                                                  "must NOT have fewer than 1 characters",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            if (!pattern4.test(data12)) {
                                              validate43.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/catalogVersion",
                                                  schemaPath:
                                                    "#/$defs/Id/pattern",
                                                  keyword: "pattern",
                                                  params: {
                                                    pattern:
                                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                                  },
                                                  message:
                                                    'must match pattern "' +
                                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                                    '"',
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                        }
                                      } else {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/catalogVersion",
                                            schemaPath: "#/$defs/Id/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                    var valid0 = _errs34 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.a2uiVersion !== undefined) {
                                      let data13 = data.a2uiVersion;
                                      const _errs37 = errors;
                                      if (typeof data13 !== "string") {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/a2uiVersion",
                                            schemaPath:
                                              "#/properties/a2uiVersion/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                      if ("v0.9.1" !== data13) {
                                        validate43.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/a2uiVersion",
                                            schemaPath:
                                              "#/properties/a2uiVersion/const",
                                            keyword: "const",
                                            params: { allowedValue: "v0.9.1" },
                                            message:
                                              "must be equal to constant",
                                          },
                                        ];
                                        return false;
                                      }
                                      var valid0 = _errs37 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.status !== undefined) {
                                        let data14 = data.status;
                                        const _errs39 = errors;
                                        if (typeof data14 !== "string") {
                                          validate43.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/status",
                                              schemaPath:
                                                "#/properties/status/type",
                                              keyword: "type",
                                              params: { type: "string" },
                                              message: "must be string",
                                            },
                                          ];
                                          return false;
                                        }
                                        if (
                                          !(
                                            data14 === "active" ||
                                            data14 === "deleted"
                                          )
                                        ) {
                                          validate43.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/status",
                                              schemaPath:
                                                "#/properties/status/enum",
                                              keyword: "enum",
                                              params: {
                                                allowedValues:
                                                  schema88.properties.status
                                                    .enum,
                                              },
                                              message:
                                                "must be equal to one of the allowed values",
                                            },
                                          ];
                                          return false;
                                        }
                                        var valid0 = _errs39 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.messages !== undefined) {
                                          let data15 = data.messages;
                                          const _errs41 = errors;
                                          if (errors === _errs41) {
                                            if (Array.isArray(data15)) {
                                              if (data15.length > 128) {
                                                validate43.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/messages",
                                                    schemaPath:
                                                      "#/properties/messages/maxItems",
                                                    keyword: "maxItems",
                                                    params: { limit: 128 },
                                                    message:
                                                      "must NOT have more than 128 items",
                                                  },
                                                ];
                                                return false;
                                              } else {
                                                var valid11 = true;
                                                const len0 = data15.length;
                                                for (
                                                  let i0 = 0;
                                                  i0 < len0;
                                                  i0++
                                                ) {
                                                  let data16 = data15[i0];
                                                  const _errs43 = errors;
                                                  if (errors === _errs43) {
                                                    if (
                                                      data16 &&
                                                      typeof data16 ==
                                                        "object" &&
                                                      !Array.isArray(data16)
                                                    ) {
                                                    } else {
                                                      validate43.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/messages/" +
                                                            i0,
                                                          schemaPath:
                                                            "#/properties/messages/items/type",
                                                          keyword: "type",
                                                          params: {
                                                            type: "object",
                                                          },
                                                          message:
                                                            "must be object",
                                                        },
                                                      ];
                                                      return false;
                                                    }
                                                  }
                                                  var valid11 =
                                                    _errs43 === errors;
                                                  if (!valid11) {
                                                    break;
                                                  }
                                                }
                                              }
                                            } else {
                                              validate43.errors = [
                                                {
                                                  instancePath:
                                                    instancePath + "/messages",
                                                  schemaPath:
                                                    "#/properties/messages/type",
                                                  keyword: "type",
                                                  params: { type: "array" },
                                                  message: "must be array",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid0 = _errs41 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate43.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate43.errors = vErrors;
  return errors === 0;
}
validate43.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate41(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate41.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.type === undefined && (missing0 = "type")) ||
        (data.messageId === undefined && (missing0 = "messageId")) ||
        (data.text === undefined && (missing0 = "text"))
      ) {
        const err0 = {
          instancePath,
          schemaPath: "#/oneOf/0/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "type" || key0 === "messageId" || key0 === "text")) {
            const err1 = {
              instancePath,
              schemaPath: "#/oneOf/0/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.type !== undefined) {
            let data0 = data.type;
            const _errs4 = errors;
            if (typeof data0 !== "string") {
              const err2 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("text" !== data0) {
              const err3 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/const",
                keyword: "const",
                params: { allowedValue: "text" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.messageId !== undefined) {
              let data1 = data.messageId;
              const _errs6 = errors;
              const _errs7 = errors;
              if (errors === _errs7) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    const err4 = {
                      instancePath: instancePath + "/messageId",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  } else {
                    if (func1(data1) < 1) {
                      const err5 = {
                        instancePath: instancePath + "/messageId",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err5];
                      } else {
                        vErrors.push(err5);
                      }
                      errors++;
                    } else {
                      if (!pattern4.test(data1)) {
                        const err6 = {
                          instancePath: instancePath + "/messageId",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      }
                    }
                  }
                } else {
                  const err7 = {
                    instancePath: instancePath + "/messageId",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
            if (valid1) {
              if (data.text !== undefined) {
                let data2 = data.text;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 65536) {
                      const err8 = {
                        instancePath: instancePath + "/text",
                        schemaPath: "#/oneOf/0/properties/text/maxLength",
                        keyword: "maxLength",
                        params: { limit: 65536 },
                        message: "must NOT have more than 65536 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err8];
                      } else {
                        vErrors.push(err8);
                      }
                      errors++;
                    }
                  } else {
                    const err9 = {
                      instancePath: instancePath + "/text",
                      schemaPath: "#/oneOf/0/properties/text/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err9];
                    } else {
                      vErrors.push(err9);
                    }
                    errors++;
                  }
                }
                var valid1 = _errs9 === errors;
              } else {
                var valid1 = true;
              }
            }
          }
        }
      }
    } else {
      const err10 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs11 = errors;
  if (errors === _errs11) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (
        (data.type === undefined && (missing1 = "type")) ||
        (data.state === undefined && (missing1 = "state"))
      ) {
        const err11 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing1 },
          message: "must have required property '" + missing1 + "'",
        };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      } else {
        const _errs13 = errors;
        for (const key1 in data) {
          if (!(key1 === "type" || key1 === "state")) {
            const err12 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key1 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err12];
            } else {
              vErrors.push(err12);
            }
            errors++;
            break;
          }
        }
        if (_errs13 === errors) {
          if (data.type !== undefined) {
            let data3 = data.type;
            const _errs14 = errors;
            if (typeof data3 !== "string") {
              const err13 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err13];
              } else {
                vErrors.push(err13);
              }
              errors++;
            }
            if ("status" !== data3) {
              const err14 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/const",
                keyword: "const",
                params: { allowedValue: "status" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err14];
              } else {
                vErrors.push(err14);
              }
              errors++;
            }
            var valid3 = _errs14 === errors;
          } else {
            var valid3 = true;
          }
          if (valid3) {
            if (data.state !== undefined) {
              let data4 = data.state;
              const _errs16 = errors;
              if (typeof data4 !== "string") {
                const err15 = {
                  instancePath: instancePath + "/state",
                  schemaPath: "#/$defs/CommandState/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              if (
                !(
                  data4 === "accepted" ||
                  data4 === "dispatching" ||
                  data4 === "running" ||
                  data4 === "terminal" ||
                  data4 === "reconciliation_required"
                )
              ) {
                const err16 = {
                  instancePath: instancePath + "/state",
                  schemaPath: "#/$defs/CommandState/enum",
                  keyword: "enum",
                  params: { allowedValues: schema59.enum },
                  message: "must be equal to one of the allowed values",
                };
                if (vErrors === null) {
                  vErrors = [err16];
                } else {
                  vErrors.push(err16);
                }
                errors++;
              }
              var valid3 = _errs16 === errors;
            } else {
              var valid3 = true;
            }
          }
        }
      }
    } else {
      const err17 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err17];
      } else {
        vErrors.push(err17);
      }
      errors++;
    }
  }
  var _valid0 = _errs11 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs19 = errors;
    if (errors === _errs19) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing2;
        if (
          (data.type === undefined && (missing2 = "type")) ||
          (data.outcome === undefined && (missing2 = "outcome"))
        ) {
          const err18 = {
            instancePath,
            schemaPath: "#/oneOf/2/required",
            keyword: "required",
            params: { missingProperty: missing2 },
            message: "must have required property '" + missing2 + "'",
          };
          if (vErrors === null) {
            vErrors = [err18];
          } else {
            vErrors.push(err18);
          }
          errors++;
        } else {
          const _errs21 = errors;
          for (const key2 in data) {
            if (!(key2 === "type" || key2 === "outcome")) {
              const err19 = {
                instancePath,
                schemaPath: "#/oneOf/2/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key2 },
                message: "must NOT have additional properties",
              };
              if (vErrors === null) {
                vErrors = [err19];
              } else {
                vErrors.push(err19);
              }
              errors++;
              break;
            }
          }
          if (_errs21 === errors) {
            if (data.type !== undefined) {
              let data5 = data.type;
              const _errs22 = errors;
              if (typeof data5 !== "string") {
                const err20 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err20];
                } else {
                  vErrors.push(err20);
                }
                errors++;
              }
              if ("terminal" !== data5) {
                const err21 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/const",
                  keyword: "const",
                  params: { allowedValue: "terminal" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err21];
                } else {
                  vErrors.push(err21);
                }
                errors++;
              }
              var valid5 = _errs22 === errors;
            } else {
              var valid5 = true;
            }
            if (valid5) {
              if (data.outcome !== undefined) {
                let data6 = data.outcome;
                const _errs24 = errors;
                if (typeof data6 !== "string") {
                  const err22 = {
                    instancePath: instancePath + "/outcome",
                    schemaPath: "#/$defs/Outcome/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err22];
                  } else {
                    vErrors.push(err22);
                  }
                  errors++;
                }
                if (
                  !(
                    data6 === "completed" ||
                    data6 === "cancelled" ||
                    data6 === "refused" ||
                    data6 === "max_tokens" ||
                    data6 === "max_turn_requests" ||
                    data6 === "failed"
                  )
                ) {
                  const err23 = {
                    instancePath: instancePath + "/outcome",
                    schemaPath: "#/$defs/Outcome/enum",
                    keyword: "enum",
                    params: { allowedValues: schema65.enum },
                    message: "must be equal to one of the allowed values",
                  };
                  if (vErrors === null) {
                    vErrors = [err23];
                  } else {
                    vErrors.push(err23);
                  }
                  errors++;
                }
                var valid5 = _errs24 === errors;
              } else {
                var valid5 = true;
              }
            }
          }
        }
      } else {
        const err24 = {
          instancePath,
          schemaPath: "#/oneOf/2/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        };
        if (vErrors === null) {
          vErrors = [err24];
        } else {
          vErrors.push(err24);
        }
        errors++;
      }
    }
    var _valid0 = _errs19 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs27 = errors;
      if (errors === _errs27) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing3;
          if (
            (data.type === undefined && (missing3 = "type")) ||
            (data.confirmation === undefined && (missing3 = "confirmation"))
          ) {
            const err25 = {
              instancePath,
              schemaPath: "#/oneOf/3/required",
              keyword: "required",
              params: { missingProperty: missing3 },
              message: "must have required property '" + missing3 + "'",
            };
            if (vErrors === null) {
              vErrors = [err25];
            } else {
              vErrors.push(err25);
            }
            errors++;
          } else {
            const _errs29 = errors;
            for (const key3 in data) {
              if (!(key3 === "type" || key3 === "confirmation")) {
                const err26 = {
                  instancePath,
                  schemaPath: "#/oneOf/3/additionalProperties",
                  keyword: "additionalProperties",
                  params: { additionalProperty: key3 },
                  message: "must NOT have additional properties",
                };
                if (vErrors === null) {
                  vErrors = [err26];
                } else {
                  vErrors.push(err26);
                }
                errors++;
                break;
              }
            }
            if (_errs29 === errors) {
              if (data.type !== undefined) {
                let data7 = data.type;
                const _errs30 = errors;
                if (typeof data7 !== "string") {
                  const err27 = {
                    instancePath: instancePath + "/type",
                    schemaPath: "#/oneOf/3/properties/type/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err27];
                  } else {
                    vErrors.push(err27);
                  }
                  errors++;
                }
                if ("cancel_dispatched" !== data7) {
                  const err28 = {
                    instancePath: instancePath + "/type",
                    schemaPath: "#/oneOf/3/properties/type/const",
                    keyword: "const",
                    params: { allowedValue: "cancel_dispatched" },
                    message: "must be equal to constant",
                  };
                  if (vErrors === null) {
                    vErrors = [err28];
                  } else {
                    vErrors.push(err28);
                  }
                  errors++;
                }
                var valid7 = _errs30 === errors;
              } else {
                var valid7 = true;
              }
              if (valid7) {
                if (data.confirmation !== undefined) {
                  let data8 = data.confirmation;
                  const _errs32 = errors;
                  if (typeof data8 !== "string") {
                    const err29 = {
                      instancePath: instancePath + "/confirmation",
                      schemaPath: "#/oneOf/3/properties/confirmation/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err29];
                    } else {
                      vErrors.push(err29);
                    }
                    errors++;
                  }
                  if (
                    !(
                      data8 === "request_only" ||
                      data8 === "already_terminal" ||
                      data8 === "unsupported"
                    )
                  ) {
                    const err30 = {
                      instancePath: instancePath + "/confirmation",
                      schemaPath: "#/oneOf/3/properties/confirmation/enum",
                      keyword: "enum",
                      params: {
                        allowedValues:
                          schema74.oneOf[3].properties.confirmation.enum,
                      },
                      message: "must be equal to one of the allowed values",
                    };
                    if (vErrors === null) {
                      vErrors = [err30];
                    } else {
                      vErrors.push(err30);
                    }
                    errors++;
                  }
                  var valid7 = _errs32 === errors;
                } else {
                  var valid7 = true;
                }
              }
            }
          }
        } else {
          const err31 = {
            instancePath,
            schemaPath: "#/oneOf/3/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          };
          if (vErrors === null) {
            vErrors = [err31];
          } else {
            vErrors.push(err31);
          }
          errors++;
        }
      }
      var _valid0 = _errs27 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs34 = errors;
        if (errors === _errs34) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing4;
            if (
              (data.type === undefined && (missing4 = "type")) ||
              (data.proposalId === undefined && (missing4 = "proposalId")) ||
              (data.name === undefined && (missing4 = "name")) ||
              (data.arguments === undefined && (missing4 = "arguments"))
            ) {
              const err32 = {
                instancePath,
                schemaPath: "#/oneOf/4/required",
                keyword: "required",
                params: { missingProperty: missing4 },
                message: "must have required property '" + missing4 + "'",
              };
              if (vErrors === null) {
                vErrors = [err32];
              } else {
                vErrors.push(err32);
              }
              errors++;
            } else {
              const _errs36 = errors;
              for (const key4 in data) {
                if (
                  !(
                    key4 === "type" ||
                    key4 === "proposalId" ||
                    key4 === "name" ||
                    key4 === "arguments"
                  )
                ) {
                  const err33 = {
                    instancePath,
                    schemaPath: "#/oneOf/4/additionalProperties",
                    keyword: "additionalProperties",
                    params: { additionalProperty: key4 },
                    message: "must NOT have additional properties",
                  };
                  if (vErrors === null) {
                    vErrors = [err33];
                  } else {
                    vErrors.push(err33);
                  }
                  errors++;
                  break;
                }
              }
              if (_errs36 === errors) {
                if (data.type !== undefined) {
                  let data9 = data.type;
                  const _errs37 = errors;
                  if (typeof data9 !== "string") {
                    const err34 = {
                      instancePath: instancePath + "/type",
                      schemaPath: "#/oneOf/4/properties/type/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err34];
                    } else {
                      vErrors.push(err34);
                    }
                    errors++;
                  }
                  if ("tool_proposal" !== data9) {
                    const err35 = {
                      instancePath: instancePath + "/type",
                      schemaPath: "#/oneOf/4/properties/type/const",
                      keyword: "const",
                      params: { allowedValue: "tool_proposal" },
                      message: "must be equal to constant",
                    };
                    if (vErrors === null) {
                      vErrors = [err35];
                    } else {
                      vErrors.push(err35);
                    }
                    errors++;
                  }
                  var valid8 = _errs37 === errors;
                } else {
                  var valid8 = true;
                }
                if (valid8) {
                  if (data.proposalId !== undefined) {
                    let data10 = data.proposalId;
                    const _errs39 = errors;
                    const _errs40 = errors;
                    if (errors === _errs40) {
                      if (typeof data10 === "string") {
                        if (func1(data10) > 128) {
                          const err36 = {
                            instancePath: instancePath + "/proposalId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err36];
                          } else {
                            vErrors.push(err36);
                          }
                          errors++;
                        } else {
                          if (func1(data10) < 1) {
                            const err37 = {
                              instancePath: instancePath + "/proposalId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            };
                            if (vErrors === null) {
                              vErrors = [err37];
                            } else {
                              vErrors.push(err37);
                            }
                            errors++;
                          } else {
                            if (!pattern4.test(data10)) {
                              const err38 = {
                                instancePath: instancePath + "/proposalId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              };
                              if (vErrors === null) {
                                vErrors = [err38];
                              } else {
                                vErrors.push(err38);
                              }
                              errors++;
                            }
                          }
                        }
                      } else {
                        const err39 = {
                          instancePath: instancePath + "/proposalId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        };
                        if (vErrors === null) {
                          vErrors = [err39];
                        } else {
                          vErrors.push(err39);
                        }
                        errors++;
                      }
                    }
                    var valid8 = _errs39 === errors;
                  } else {
                    var valid8 = true;
                  }
                  if (valid8) {
                    if (data.name !== undefined) {
                      let data11 = data.name;
                      const _errs42 = errors;
                      const _errs43 = errors;
                      if (errors === _errs43) {
                        if (typeof data11 === "string") {
                          if (func1(data11) > 128) {
                            const err40 = {
                              instancePath: instancePath + "/name",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            };
                            if (vErrors === null) {
                              vErrors = [err40];
                            } else {
                              vErrors.push(err40);
                            }
                            errors++;
                          } else {
                            if (func1(data11) < 1) {
                              const err41 = {
                                instancePath: instancePath + "/name",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              };
                              if (vErrors === null) {
                                vErrors = [err41];
                              } else {
                                vErrors.push(err41);
                              }
                              errors++;
                            } else {
                              if (!pattern4.test(data11)) {
                                const err42 = {
                                  instancePath: instancePath + "/name",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                };
                                if (vErrors === null) {
                                  vErrors = [err42];
                                } else {
                                  vErrors.push(err42);
                                }
                                errors++;
                              }
                            }
                          }
                        } else {
                          const err43 = {
                            instancePath: instancePath + "/name",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          };
                          if (vErrors === null) {
                            vErrors = [err43];
                          } else {
                            vErrors.push(err43);
                          }
                          errors++;
                        }
                      }
                      var valid8 = _errs42 === errors;
                    } else {
                      var valid8 = true;
                    }
                    if (valid8) {
                      if (data.arguments !== undefined) {
                        let data12 = data.arguments;
                        const _errs45 = errors;
                        if (errors === _errs45) {
                          if (
                            data12 &&
                            typeof data12 == "object" &&
                            !Array.isArray(data12)
                          ) {
                          } else {
                            const err44 = {
                              instancePath: instancePath + "/arguments",
                              schemaPath: "#/oneOf/4/properties/arguments/type",
                              keyword: "type",
                              params: { type: "object" },
                              message: "must be object",
                            };
                            if (vErrors === null) {
                              vErrors = [err44];
                            } else {
                              vErrors.push(err44);
                            }
                            errors++;
                          }
                        }
                        var valid8 = _errs45 === errors;
                      } else {
                        var valid8 = true;
                      }
                    }
                  }
                }
              }
            }
          } else {
            const err45 = {
              instancePath,
              schemaPath: "#/oneOf/4/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            };
            if (vErrors === null) {
              vErrors = [err45];
            } else {
              vErrors.push(err45);
            }
            errors++;
          }
        }
        var _valid0 = _errs34 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs48 = errors;
          if (errors === _errs48) {
            if (data && typeof data == "object" && !Array.isArray(data)) {
              let missing5;
              if (
                (data.type === undefined && (missing5 = "type")) ||
                (data.proposalId === undefined && (missing5 = "proposalId")) ||
                (data.disposition === undefined &&
                  (missing5 = "disposition")) ||
                (data.text === undefined && (missing5 = "text"))
              ) {
                const err46 = {
                  instancePath,
                  schemaPath: "#/oneOf/5/required",
                  keyword: "required",
                  params: { missingProperty: missing5 },
                  message: "must have required property '" + missing5 + "'",
                };
                if (vErrors === null) {
                  vErrors = [err46];
                } else {
                  vErrors.push(err46);
                }
                errors++;
              } else {
                const _errs50 = errors;
                for (const key5 in data) {
                  if (
                    !(
                      key5 === "type" ||
                      key5 === "proposalId" ||
                      key5 === "disposition" ||
                      key5 === "text"
                    )
                  ) {
                    const err47 = {
                      instancePath,
                      schemaPath: "#/oneOf/5/additionalProperties",
                      keyword: "additionalProperties",
                      params: { additionalProperty: key5 },
                      message: "must NOT have additional properties",
                    };
                    if (vErrors === null) {
                      vErrors = [err47];
                    } else {
                      vErrors.push(err47);
                    }
                    errors++;
                    break;
                  }
                }
                if (_errs50 === errors) {
                  if (data.type !== undefined) {
                    let data13 = data.type;
                    const _errs51 = errors;
                    if (typeof data13 !== "string") {
                      const err48 = {
                        instancePath: instancePath + "/type",
                        schemaPath: "#/oneOf/5/properties/type/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err48];
                      } else {
                        vErrors.push(err48);
                      }
                      errors++;
                    }
                    if ("tool_result" !== data13) {
                      const err49 = {
                        instancePath: instancePath + "/type",
                        schemaPath: "#/oneOf/5/properties/type/const",
                        keyword: "const",
                        params: { allowedValue: "tool_result" },
                        message: "must be equal to constant",
                      };
                      if (vErrors === null) {
                        vErrors = [err49];
                      } else {
                        vErrors.push(err49);
                      }
                      errors++;
                    }
                    var valid11 = _errs51 === errors;
                  } else {
                    var valid11 = true;
                  }
                  if (valid11) {
                    if (data.proposalId !== undefined) {
                      let data14 = data.proposalId;
                      const _errs53 = errors;
                      const _errs54 = errors;
                      if (errors === _errs54) {
                        if (typeof data14 === "string") {
                          if (func1(data14) > 128) {
                            const err50 = {
                              instancePath: instancePath + "/proposalId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            };
                            if (vErrors === null) {
                              vErrors = [err50];
                            } else {
                              vErrors.push(err50);
                            }
                            errors++;
                          } else {
                            if (func1(data14) < 1) {
                              const err51 = {
                                instancePath: instancePath + "/proposalId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              };
                              if (vErrors === null) {
                                vErrors = [err51];
                              } else {
                                vErrors.push(err51);
                              }
                              errors++;
                            } else {
                              if (!pattern4.test(data14)) {
                                const err52 = {
                                  instancePath: instancePath + "/proposalId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                };
                                if (vErrors === null) {
                                  vErrors = [err52];
                                } else {
                                  vErrors.push(err52);
                                }
                                errors++;
                              }
                            }
                          }
                        } else {
                          const err53 = {
                            instancePath: instancePath + "/proposalId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          };
                          if (vErrors === null) {
                            vErrors = [err53];
                          } else {
                            vErrors.push(err53);
                          }
                          errors++;
                        }
                      }
                      var valid11 = _errs53 === errors;
                    } else {
                      var valid11 = true;
                    }
                    if (valid11) {
                      if (data.disposition !== undefined) {
                        let data15 = data.disposition;
                        const _errs56 = errors;
                        if (typeof data15 !== "string") {
                          const err54 = {
                            instancePath: instancePath + "/disposition",
                            schemaPath: "#/oneOf/5/properties/disposition/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          };
                          if (vErrors === null) {
                            vErrors = [err54];
                          } else {
                            vErrors.push(err54);
                          }
                          errors++;
                        }
                        if (
                          !(
                            data15 === "returned" ||
                            data15 === "rejected" ||
                            data15 === "unavailable"
                          )
                        ) {
                          const err55 = {
                            instancePath: instancePath + "/disposition",
                            schemaPath: "#/oneOf/5/properties/disposition/enum",
                            keyword: "enum",
                            params: {
                              allowedValues:
                                schema74.oneOf[5].properties.disposition.enum,
                            },
                            message:
                              "must be equal to one of the allowed values",
                          };
                          if (vErrors === null) {
                            vErrors = [err55];
                          } else {
                            vErrors.push(err55);
                          }
                          errors++;
                        }
                        var valid11 = _errs56 === errors;
                      } else {
                        var valid11 = true;
                      }
                      if (valid11) {
                        if (data.text !== undefined) {
                          let data16 = data.text;
                          const _errs58 = errors;
                          if (errors === _errs58) {
                            if (typeof data16 === "string") {
                              if (func1(data16) > 65536) {
                                const err56 = {
                                  instancePath: instancePath + "/text",
                                  schemaPath:
                                    "#/oneOf/5/properties/text/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 65536 },
                                  message:
                                    "must NOT have more than 65536 characters",
                                };
                                if (vErrors === null) {
                                  vErrors = [err56];
                                } else {
                                  vErrors.push(err56);
                                }
                                errors++;
                              }
                            } else {
                              const err57 = {
                                instancePath: instancePath + "/text",
                                schemaPath: "#/oneOf/5/properties/text/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              };
                              if (vErrors === null) {
                                vErrors = [err57];
                              } else {
                                vErrors.push(err57);
                              }
                              errors++;
                            }
                          }
                          var valid11 = _errs58 === errors;
                        } else {
                          var valid11 = true;
                        }
                      }
                    }
                  }
                }
              }
            } else {
              const err58 = {
                instancePath,
                schemaPath: "#/oneOf/5/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              };
              if (vErrors === null) {
                vErrors = [err58];
              } else {
                vErrors.push(err58);
              }
              errors++;
            }
          }
          var _valid0 = _errs48 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs60 = errors;
            if (errors === _errs60) {
              if (data && typeof data == "object" && !Array.isArray(data)) {
                let missing6;
                if (
                  (data.type === undefined && (missing6 = "type")) ||
                  (data.interactionId === undefined &&
                    (missing6 = "interactionId")) ||
                  (data.status === undefined && (missing6 = "status")) ||
                  (data.request === undefined && (missing6 = "request")) ||
                  (data.expiresAtMs === undefined &&
                    (missing6 = "expiresAtMs")) ||
                  (data.callbackLifetime === undefined &&
                    (missing6 = "callbackLifetime"))
                ) {
                  const err59 = {
                    instancePath,
                    schemaPath: "#/oneOf/6/required",
                    keyword: "required",
                    params: { missingProperty: missing6 },
                    message: "must have required property '" + missing6 + "'",
                  };
                  if (vErrors === null) {
                    vErrors = [err59];
                  } else {
                    vErrors.push(err59);
                  }
                  errors++;
                } else {
                  const _errs62 = errors;
                  for (const key6 in data) {
                    if (
                      !(
                        key6 === "type" ||
                        key6 === "interactionId" ||
                        key6 === "status" ||
                        key6 === "request" ||
                        key6 === "expiresAtMs" ||
                        key6 === "callbackLifetime"
                      )
                    ) {
                      const err60 = {
                        instancePath,
                        schemaPath: "#/oneOf/6/additionalProperties",
                        keyword: "additionalProperties",
                        params: { additionalProperty: key6 },
                        message: "must NOT have additional properties",
                      };
                      if (vErrors === null) {
                        vErrors = [err60];
                      } else {
                        vErrors.push(err60);
                      }
                      errors++;
                      break;
                    }
                  }
                  if (_errs62 === errors) {
                    if (data.type !== undefined) {
                      let data17 = data.type;
                      const _errs63 = errors;
                      if (typeof data17 !== "string") {
                        const err61 = {
                          instancePath: instancePath + "/type",
                          schemaPath: "#/oneOf/6/properties/type/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        };
                        if (vErrors === null) {
                          vErrors = [err61];
                        } else {
                          vErrors.push(err61);
                        }
                        errors++;
                      }
                      if ("interaction" !== data17) {
                        const err62 = {
                          instancePath: instancePath + "/type",
                          schemaPath: "#/oneOf/6/properties/type/const",
                          keyword: "const",
                          params: { allowedValue: "interaction" },
                          message: "must be equal to constant",
                        };
                        if (vErrors === null) {
                          vErrors = [err62];
                        } else {
                          vErrors.push(err62);
                        }
                        errors++;
                      }
                      var valid13 = _errs63 === errors;
                    } else {
                      var valid13 = true;
                    }
                    if (valid13) {
                      if (data.interactionId !== undefined) {
                        let data18 = data.interactionId;
                        const _errs65 = errors;
                        const _errs66 = errors;
                        if (errors === _errs66) {
                          if (typeof data18 === "string") {
                            if (func1(data18) > 128) {
                              const err63 = {
                                instancePath: instancePath + "/interactionId",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              };
                              if (vErrors === null) {
                                vErrors = [err63];
                              } else {
                                vErrors.push(err63);
                              }
                              errors++;
                            } else {
                              if (func1(data18) < 1) {
                                const err64 = {
                                  instancePath: instancePath + "/interactionId",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                };
                                if (vErrors === null) {
                                  vErrors = [err64];
                                } else {
                                  vErrors.push(err64);
                                }
                                errors++;
                              } else {
                                if (!pattern4.test(data18)) {
                                  const err65 = {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err65];
                                  } else {
                                    vErrors.push(err65);
                                  }
                                  errors++;
                                }
                              }
                            }
                          } else {
                            const err66 = {
                              instancePath: instancePath + "/interactionId",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            };
                            if (vErrors === null) {
                              vErrors = [err66];
                            } else {
                              vErrors.push(err66);
                            }
                            errors++;
                          }
                        }
                        var valid13 = _errs65 === errors;
                      } else {
                        var valid13 = true;
                      }
                      if (valid13) {
                        if (data.status !== undefined) {
                          let data19 = data.status;
                          const _errs68 = errors;
                          if (typeof data19 !== "string") {
                            const err67 = {
                              instancePath: instancePath + "/status",
                              schemaPath: "#/oneOf/6/properties/status/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            };
                            if (vErrors === null) {
                              vErrors = [err67];
                            } else {
                              vErrors.push(err67);
                            }
                            errors++;
                          }
                          if ("pending" !== data19) {
                            const err68 = {
                              instancePath: instancePath + "/status",
                              schemaPath: "#/oneOf/6/properties/status/const",
                              keyword: "const",
                              params: { allowedValue: "pending" },
                              message: "must be equal to constant",
                            };
                            if (vErrors === null) {
                              vErrors = [err68];
                            } else {
                              vErrors.push(err68);
                            }
                            errors++;
                          }
                          var valid13 = _errs68 === errors;
                        } else {
                          var valid13 = true;
                        }
                        if (valid13) {
                          if (data.request !== undefined) {
                            let data20 = data.request;
                            const _errs70 = errors;
                            const _errs71 = errors;
                            if (errors === _errs71) {
                              if (
                                data20 &&
                                typeof data20 == "object" &&
                                !Array.isArray(data20)
                              ) {
                              } else {
                                const err69 = {
                                  instancePath: instancePath + "/request",
                                  schemaPath: "#/$defs/InteractionRequest/type",
                                  keyword: "type",
                                  params: { type: "object" },
                                  message: "must be object",
                                };
                                if (vErrors === null) {
                                  vErrors = [err69];
                                } else {
                                  vErrors.push(err69);
                                }
                                errors++;
                              }
                            }
                            var valid13 = _errs70 === errors;
                          } else {
                            var valid13 = true;
                          }
                          if (valid13) {
                            if (data.expiresAtMs !== undefined) {
                              let data21 = data.expiresAtMs;
                              const _errs74 = errors;
                              const _errs75 = errors;
                              if (
                                !(
                                  typeof data21 == "number" &&
                                  !(data21 % 1) &&
                                  !isNaN(data21) &&
                                  isFinite(data21)
                                )
                              ) {
                                const err70 = {
                                  instancePath: instancePath + "/expiresAtMs",
                                  schemaPath: "#/$defs/Counter/type",
                                  keyword: "type",
                                  params: { type: "integer" },
                                  message: "must be integer",
                                };
                                if (vErrors === null) {
                                  vErrors = [err70];
                                } else {
                                  vErrors.push(err70);
                                }
                                errors++;
                              }
                              if (errors === _errs75) {
                                if (
                                  typeof data21 == "number" &&
                                  isFinite(data21)
                                ) {
                                  if (
                                    data21 > 9007199254740991 ||
                                    isNaN(data21)
                                  ) {
                                    const err71 = {
                                      instancePath:
                                        instancePath + "/expiresAtMs",
                                      schemaPath: "#/$defs/Counter/maximum",
                                      keyword: "maximum",
                                      params: {
                                        comparison: "<=",
                                        limit: 9007199254740991,
                                      },
                                      message: "must be <= 9007199254740991",
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err71];
                                    } else {
                                      vErrors.push(err71);
                                    }
                                    errors++;
                                  } else {
                                    if (data21 < 0 || isNaN(data21)) {
                                      const err72 = {
                                        instancePath:
                                          instancePath + "/expiresAtMs",
                                        schemaPath: "#/$defs/Counter/minimum",
                                        keyword: "minimum",
                                        params: { comparison: ">=", limit: 0 },
                                        message: "must be >= 0",
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err72];
                                      } else {
                                        vErrors.push(err72);
                                      }
                                      errors++;
                                    }
                                  }
                                }
                              }
                              var valid13 = _errs74 === errors;
                            } else {
                              var valid13 = true;
                            }
                            if (valid13) {
                              if (data.callbackLifetime !== undefined) {
                                let data22 = data.callbackLifetime;
                                const _errs77 = errors;
                                if (typeof data22 !== "string") {
                                  const err73 = {
                                    instancePath:
                                      instancePath + "/callbackLifetime",
                                    schemaPath: "#/$defs/CallbackLifetime/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err73];
                                  } else {
                                    vErrors.push(err73);
                                  }
                                  errors++;
                                }
                                if (
                                  !(
                                    data22 === "generation_bound" ||
                                    data22 === "provider_resumable"
                                  )
                                ) {
                                  const err74 = {
                                    instancePath:
                                      instancePath + "/callbackLifetime",
                                    schemaPath: "#/$defs/CallbackLifetime/enum",
                                    keyword: "enum",
                                    params: { allowedValues: schema84.enum },
                                    message:
                                      "must be equal to one of the allowed values",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err74];
                                  } else {
                                    vErrors.push(err74);
                                  }
                                  errors++;
                                }
                                var valid13 = _errs77 === errors;
                              } else {
                                var valid13 = true;
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                const err75 = {
                  instancePath,
                  schemaPath: "#/oneOf/6/type",
                  keyword: "type",
                  params: { type: "object" },
                  message: "must be object",
                };
                if (vErrors === null) {
                  vErrors = [err75];
                } else {
                  vErrors.push(err75);
                }
                errors++;
              }
            }
            var _valid0 = _errs60 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs80 = errors;
              if (errors === _errs80) {
                if (data && typeof data == "object" && !Array.isArray(data)) {
                  let missing7;
                  if (
                    (data.type === undefined && (missing7 = "type")) ||
                    (data.interactionId === undefined &&
                      (missing7 = "interactionId")) ||
                    (data.status === undefined && (missing7 = "status")) ||
                    (data.responseCommandId === undefined &&
                      (missing7 = "responseCommandId"))
                  ) {
                    const err76 = {
                      instancePath,
                      schemaPath: "#/oneOf/7/required",
                      keyword: "required",
                      params: { missingProperty: missing7 },
                      message: "must have required property '" + missing7 + "'",
                    };
                    if (vErrors === null) {
                      vErrors = [err76];
                    } else {
                      vErrors.push(err76);
                    }
                    errors++;
                  } else {
                    const _errs82 = errors;
                    for (const key7 in data) {
                      if (
                        !(
                          key7 === "type" ||
                          key7 === "interactionId" ||
                          key7 === "status" ||
                          key7 === "responseCommandId"
                        )
                      ) {
                        const err77 = {
                          instancePath,
                          schemaPath: "#/oneOf/7/additionalProperties",
                          keyword: "additionalProperties",
                          params: { additionalProperty: key7 },
                          message: "must NOT have additional properties",
                        };
                        if (vErrors === null) {
                          vErrors = [err77];
                        } else {
                          vErrors.push(err77);
                        }
                        errors++;
                        break;
                      }
                    }
                    if (_errs82 === errors) {
                      if (data.type !== undefined) {
                        let data23 = data.type;
                        const _errs83 = errors;
                        if (typeof data23 !== "string") {
                          const err78 = {
                            instancePath: instancePath + "/type",
                            schemaPath: "#/oneOf/7/properties/type/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          };
                          if (vErrors === null) {
                            vErrors = [err78];
                          } else {
                            vErrors.push(err78);
                          }
                          errors++;
                        }
                        if ("interaction" !== data23) {
                          const err79 = {
                            instancePath: instancePath + "/type",
                            schemaPath: "#/oneOf/7/properties/type/const",
                            keyword: "const",
                            params: { allowedValue: "interaction" },
                            message: "must be equal to constant",
                          };
                          if (vErrors === null) {
                            vErrors = [err79];
                          } else {
                            vErrors.push(err79);
                          }
                          errors++;
                        }
                        var valid18 = _errs83 === errors;
                      } else {
                        var valid18 = true;
                      }
                      if (valid18) {
                        if (data.interactionId !== undefined) {
                          let data24 = data.interactionId;
                          const _errs85 = errors;
                          const _errs86 = errors;
                          if (errors === _errs86) {
                            if (typeof data24 === "string") {
                              if (func1(data24) > 128) {
                                const err80 = {
                                  instancePath: instancePath + "/interactionId",
                                  schemaPath: "#/$defs/Id/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 characters",
                                };
                                if (vErrors === null) {
                                  vErrors = [err80];
                                } else {
                                  vErrors.push(err80);
                                }
                                errors++;
                              } else {
                                if (func1(data24) < 1) {
                                  const err81 = {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath: "#/$defs/Id/minLength",
                                    keyword: "minLength",
                                    params: { limit: 1 },
                                    message:
                                      "must NOT have fewer than 1 characters",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err81];
                                  } else {
                                    vErrors.push(err81);
                                  }
                                  errors++;
                                } else {
                                  if (!pattern4.test(data24)) {
                                    const err82 = {
                                      instancePath:
                                        instancePath + "/interactionId",
                                      schemaPath: "#/$defs/Id/pattern",
                                      keyword: "pattern",
                                      params: {
                                        pattern:
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                      },
                                      message:
                                        'must match pattern "' +
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                        '"',
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err82];
                                    } else {
                                      vErrors.push(err82);
                                    }
                                    errors++;
                                  }
                                }
                              }
                            } else {
                              const err83 = {
                                instancePath: instancePath + "/interactionId",
                                schemaPath: "#/$defs/Id/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              };
                              if (vErrors === null) {
                                vErrors = [err83];
                              } else {
                                vErrors.push(err83);
                              }
                              errors++;
                            }
                          }
                          var valid18 = _errs85 === errors;
                        } else {
                          var valid18 = true;
                        }
                        if (valid18) {
                          if (data.status !== undefined) {
                            let data25 = data.status;
                            const _errs88 = errors;
                            if (typeof data25 !== "string") {
                              const err84 = {
                                instancePath: instancePath + "/status",
                                schemaPath: "#/oneOf/7/properties/status/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              };
                              if (vErrors === null) {
                                vErrors = [err84];
                              } else {
                                vErrors.push(err84);
                              }
                              errors++;
                            }
                            if ("answered" !== data25) {
                              const err85 = {
                                instancePath: instancePath + "/status",
                                schemaPath: "#/oneOf/7/properties/status/const",
                                keyword: "const",
                                params: { allowedValue: "answered" },
                                message: "must be equal to constant",
                              };
                              if (vErrors === null) {
                                vErrors = [err85];
                              } else {
                                vErrors.push(err85);
                              }
                              errors++;
                            }
                            var valid18 = _errs88 === errors;
                          } else {
                            var valid18 = true;
                          }
                          if (valid18) {
                            if (data.responseCommandId !== undefined) {
                              let data26 = data.responseCommandId;
                              const _errs90 = errors;
                              const _errs91 = errors;
                              if (errors === _errs91) {
                                if (typeof data26 === "string") {
                                  if (func1(data26) > 128) {
                                    const err86 = {
                                      instancePath:
                                        instancePath + "/responseCommandId",
                                      schemaPath: "#/$defs/Id/maxLength",
                                      keyword: "maxLength",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 characters",
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err86];
                                    } else {
                                      vErrors.push(err86);
                                    }
                                    errors++;
                                  } else {
                                    if (func1(data26) < 1) {
                                      const err87 = {
                                        instancePath:
                                          instancePath + "/responseCommandId",
                                        schemaPath: "#/$defs/Id/minLength",
                                        keyword: "minLength",
                                        params: { limit: 1 },
                                        message:
                                          "must NOT have fewer than 1 characters",
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err87];
                                      } else {
                                        vErrors.push(err87);
                                      }
                                      errors++;
                                    } else {
                                      if (!pattern4.test(data26)) {
                                        const err88 = {
                                          instancePath:
                                            instancePath + "/responseCommandId",
                                          schemaPath: "#/$defs/Id/pattern",
                                          keyword: "pattern",
                                          params: {
                                            pattern:
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                          },
                                          message:
                                            'must match pattern "' +
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                            '"',
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err88];
                                        } else {
                                          vErrors.push(err88);
                                        }
                                        errors++;
                                      }
                                    }
                                  }
                                } else {
                                  const err89 = {
                                    instancePath:
                                      instancePath + "/responseCommandId",
                                    schemaPath: "#/$defs/Id/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err89];
                                  } else {
                                    vErrors.push(err89);
                                  }
                                  errors++;
                                }
                              }
                              var valid18 = _errs90 === errors;
                            } else {
                              var valid18 = true;
                            }
                          }
                        }
                      }
                    }
                  }
                } else {
                  const err90 = {
                    instancePath,
                    schemaPath: "#/oneOf/7/type",
                    keyword: "type",
                    params: { type: "object" },
                    message: "must be object",
                  };
                  if (vErrors === null) {
                    vErrors = [err90];
                  } else {
                    vErrors.push(err90);
                  }
                  errors++;
                }
              }
              var _valid0 = _errs80 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs93 = errors;
                if (errors === _errs93) {
                  if (data && typeof data == "object" && !Array.isArray(data)) {
                    let missing8;
                    if (
                      (data.type === undefined && (missing8 = "type")) ||
                      (data.interactionId === undefined &&
                        (missing8 = "interactionId")) ||
                      (data.status === undefined && (missing8 = "status"))
                    ) {
                      const err91 = {
                        instancePath,
                        schemaPath: "#/oneOf/8/required",
                        keyword: "required",
                        params: { missingProperty: missing8 },
                        message:
                          "must have required property '" + missing8 + "'",
                      };
                      if (vErrors === null) {
                        vErrors = [err91];
                      } else {
                        vErrors.push(err91);
                      }
                      errors++;
                    } else {
                      const _errs95 = errors;
                      for (const key8 in data) {
                        if (
                          !(
                            key8 === "type" ||
                            key8 === "interactionId" ||
                            key8 === "status"
                          )
                        ) {
                          const err92 = {
                            instancePath,
                            schemaPath: "#/oneOf/8/additionalProperties",
                            keyword: "additionalProperties",
                            params: { additionalProperty: key8 },
                            message: "must NOT have additional properties",
                          };
                          if (vErrors === null) {
                            vErrors = [err92];
                          } else {
                            vErrors.push(err92);
                          }
                          errors++;
                          break;
                        }
                      }
                      if (_errs95 === errors) {
                        if (data.type !== undefined) {
                          let data27 = data.type;
                          const _errs96 = errors;
                          if (typeof data27 !== "string") {
                            const err93 = {
                              instancePath: instancePath + "/type",
                              schemaPath: "#/oneOf/8/properties/type/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            };
                            if (vErrors === null) {
                              vErrors = [err93];
                            } else {
                              vErrors.push(err93);
                            }
                            errors++;
                          }
                          if ("interaction" !== data27) {
                            const err94 = {
                              instancePath: instancePath + "/type",
                              schemaPath: "#/oneOf/8/properties/type/const",
                              keyword: "const",
                              params: { allowedValue: "interaction" },
                              message: "must be equal to constant",
                            };
                            if (vErrors === null) {
                              vErrors = [err94];
                            } else {
                              vErrors.push(err94);
                            }
                            errors++;
                          }
                          var valid21 = _errs96 === errors;
                        } else {
                          var valid21 = true;
                        }
                        if (valid21) {
                          if (data.interactionId !== undefined) {
                            let data28 = data.interactionId;
                            const _errs98 = errors;
                            const _errs99 = errors;
                            if (errors === _errs99) {
                              if (typeof data28 === "string") {
                                if (func1(data28) > 128) {
                                  const err95 = {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath: "#/$defs/Id/maxLength",
                                    keyword: "maxLength",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 characters",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err95];
                                  } else {
                                    vErrors.push(err95);
                                  }
                                  errors++;
                                } else {
                                  if (func1(data28) < 1) {
                                    const err96 = {
                                      instancePath:
                                        instancePath + "/interactionId",
                                      schemaPath: "#/$defs/Id/minLength",
                                      keyword: "minLength",
                                      params: { limit: 1 },
                                      message:
                                        "must NOT have fewer than 1 characters",
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err96];
                                    } else {
                                      vErrors.push(err96);
                                    }
                                    errors++;
                                  } else {
                                    if (!pattern4.test(data28)) {
                                      const err97 = {
                                        instancePath:
                                          instancePath + "/interactionId",
                                        schemaPath: "#/$defs/Id/pattern",
                                        keyword: "pattern",
                                        params: {
                                          pattern:
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                        },
                                        message:
                                          'must match pattern "' +
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                          '"',
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err97];
                                      } else {
                                        vErrors.push(err97);
                                      }
                                      errors++;
                                    }
                                  }
                                }
                              } else {
                                const err98 = {
                                  instancePath: instancePath + "/interactionId",
                                  schemaPath: "#/$defs/Id/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                };
                                if (vErrors === null) {
                                  vErrors = [err98];
                                } else {
                                  vErrors.push(err98);
                                }
                                errors++;
                              }
                            }
                            var valid21 = _errs98 === errors;
                          } else {
                            var valid21 = true;
                          }
                          if (valid21) {
                            if (data.status !== undefined) {
                              let data29 = data.status;
                              const _errs101 = errors;
                              if (typeof data29 !== "string") {
                                const err99 = {
                                  instancePath: instancePath + "/status",
                                  schemaPath:
                                    "#/oneOf/8/properties/status/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                };
                                if (vErrors === null) {
                                  vErrors = [err99];
                                } else {
                                  vErrors.push(err99);
                                }
                                errors++;
                              }
                              if (
                                !(
                                  data29 === "expired" ||
                                  data29 === "unavailable"
                                )
                              ) {
                                const err100 = {
                                  instancePath: instancePath + "/status",
                                  schemaPath:
                                    "#/oneOf/8/properties/status/enum",
                                  keyword: "enum",
                                  params: {
                                    allowedValues:
                                      schema74.oneOf[8].properties.status.enum,
                                  },
                                  message:
                                    "must be equal to one of the allowed values",
                                };
                                if (vErrors === null) {
                                  vErrors = [err100];
                                } else {
                                  vErrors.push(err100);
                                }
                                errors++;
                              }
                              var valid21 = _errs101 === errors;
                            } else {
                              var valid21 = true;
                            }
                          }
                        }
                      }
                    }
                  } else {
                    const err101 = {
                      instancePath,
                      schemaPath: "#/oneOf/8/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
                    };
                    if (vErrors === null) {
                      vErrors = [err101];
                    } else {
                      vErrors.push(err101);
                    }
                    errors++;
                  }
                }
                var _valid0 = _errs93 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs103 = errors;
                  if (errors === _errs103) {
                    if (
                      data &&
                      typeof data == "object" &&
                      !Array.isArray(data)
                    ) {
                      let missing9;
                      if (
                        (data.type === undefined && (missing9 = "type")) ||
                        (data.failure === undefined && (missing9 = "failure"))
                      ) {
                        const err102 = {
                          instancePath,
                          schemaPath: "#/oneOf/9/required",
                          keyword: "required",
                          params: { missingProperty: missing9 },
                          message:
                            "must have required property '" + missing9 + "'",
                        };
                        if (vErrors === null) {
                          vErrors = [err102];
                        } else {
                          vErrors.push(err102);
                        }
                        errors++;
                      } else {
                        const _errs105 = errors;
                        for (const key9 in data) {
                          if (!(key9 === "type" || key9 === "failure")) {
                            const err103 = {
                              instancePath,
                              schemaPath: "#/oneOf/9/additionalProperties",
                              keyword: "additionalProperties",
                              params: { additionalProperty: key9 },
                              message: "must NOT have additional properties",
                            };
                            if (vErrors === null) {
                              vErrors = [err103];
                            } else {
                              vErrors.push(err103);
                            }
                            errors++;
                            break;
                          }
                        }
                        if (_errs105 === errors) {
                          if (data.type !== undefined) {
                            let data30 = data.type;
                            const _errs106 = errors;
                            if (typeof data30 !== "string") {
                              const err104 = {
                                instancePath: instancePath + "/type",
                                schemaPath: "#/oneOf/9/properties/type/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              };
                              if (vErrors === null) {
                                vErrors = [err104];
                              } else {
                                vErrors.push(err104);
                              }
                              errors++;
                            }
                            if ("error" !== data30) {
                              const err105 = {
                                instancePath: instancePath + "/type",
                                schemaPath: "#/oneOf/9/properties/type/const",
                                keyword: "const",
                                params: { allowedValue: "error" },
                                message: "must be equal to constant",
                              };
                              if (vErrors === null) {
                                vErrors = [err105];
                              } else {
                                vErrors.push(err105);
                              }
                              errors++;
                            }
                            var valid23 = _errs106 === errors;
                          } else {
                            var valid23 = true;
                          }
                          if (valid23) {
                            if (data.failure !== undefined) {
                              const _errs108 = errors;
                              if (
                                !validate36(data.failure, {
                                  instancePath: instancePath + "/failure",
                                  parentData: data,
                                  parentDataProperty: "failure",
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? validate36.errors
                                    : vErrors.concat(validate36.errors);
                                errors = vErrors.length;
                              }
                              var valid23 = _errs108 === errors;
                            } else {
                              var valid23 = true;
                            }
                          }
                        }
                      }
                    } else {
                      const err106 = {
                        instancePath,
                        schemaPath: "#/oneOf/9/type",
                        keyword: "type",
                        params: { type: "object" },
                        message: "must be object",
                      };
                      if (vErrors === null) {
                        vErrors = [err106];
                      } else {
                        vErrors.push(err106);
                      }
                      errors++;
                    }
                  }
                  var _valid0 = _errs103 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs109 = errors;
                    if (errors === _errs109) {
                      if (
                        data &&
                        typeof data == "object" &&
                        !Array.isArray(data)
                      ) {
                        let missing10;
                        if (
                          (data.type === undefined && (missing10 = "type")) ||
                          (data.surface === undefined &&
                            (missing10 = "surface"))
                        ) {
                          const err107 = {
                            instancePath,
                            schemaPath: "#/oneOf/10/required",
                            keyword: "required",
                            params: { missingProperty: missing10 },
                            message:
                              "must have required property '" + missing10 + "'",
                          };
                          if (vErrors === null) {
                            vErrors = [err107];
                          } else {
                            vErrors.push(err107);
                          }
                          errors++;
                        } else {
                          const _errs111 = errors;
                          for (const key10 in data) {
                            if (!(key10 === "type" || key10 === "surface")) {
                              const err108 = {
                                instancePath,
                                schemaPath: "#/oneOf/10/additionalProperties",
                                keyword: "additionalProperties",
                                params: { additionalProperty: key10 },
                                message: "must NOT have additional properties",
                              };
                              if (vErrors === null) {
                                vErrors = [err108];
                              } else {
                                vErrors.push(err108);
                              }
                              errors++;
                              break;
                            }
                          }
                          if (_errs111 === errors) {
                            if (data.type !== undefined) {
                              let data32 = data.type;
                              const _errs112 = errors;
                              if (typeof data32 !== "string") {
                                const err109 = {
                                  instancePath: instancePath + "/type",
                                  schemaPath: "#/oneOf/10/properties/type/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                };
                                if (vErrors === null) {
                                  vErrors = [err109];
                                } else {
                                  vErrors.push(err109);
                                }
                                errors++;
                              }
                              if ("surface" !== data32) {
                                const err110 = {
                                  instancePath: instancePath + "/type",
                                  schemaPath:
                                    "#/oneOf/10/properties/type/const",
                                  keyword: "const",
                                  params: { allowedValue: "surface" },
                                  message: "must be equal to constant",
                                };
                                if (vErrors === null) {
                                  vErrors = [err110];
                                } else {
                                  vErrors.push(err110);
                                }
                                errors++;
                              }
                              var valid24 = _errs112 === errors;
                            } else {
                              var valid24 = true;
                            }
                            if (valid24) {
                              if (data.surface !== undefined) {
                                const _errs114 = errors;
                                if (
                                  !validate43(data.surface, {
                                    instancePath: instancePath + "/surface",
                                    parentData: data,
                                    parentDataProperty: "surface",
                                    rootData,
                                    dynamicAnchors,
                                  })
                                ) {
                                  vErrors =
                                    vErrors === null
                                      ? validate43.errors
                                      : vErrors.concat(validate43.errors);
                                  errors = vErrors.length;
                                }
                                var valid24 = _errs114 === errors;
                              } else {
                                var valid24 = true;
                              }
                            }
                          }
                        }
                      } else {
                        const err111 = {
                          instancePath,
                          schemaPath: "#/oneOf/10/type",
                          keyword: "type",
                          params: { type: "object" },
                          message: "must be object",
                        };
                        if (vErrors === null) {
                          vErrors = [err111];
                        } else {
                          vErrors.push(err111);
                        }
                        errors++;
                      }
                    }
                    var _valid0 = _errs109 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err112 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err112];
    } else {
      vErrors.push(err112);
    }
    errors++;
    validate41.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate41.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate41.evaluated = { dynamicProps: true, dynamicItems: false };
function validate39(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate39.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.eventId === undefined && (missing0 = "eventId")) ||
        (data.sequence === undefined && (missing0 = "sequence")) ||
        (data.commandId === undefined && (missing0 = "commandId")) ||
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.body === undefined && (missing0 = "body"))
      ) {
        validate39.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "namespace" ||
              key0 === "eventId" ||
              key0 === "sequence" ||
              key0 === "commandId" ||
              key0 === "generation" ||
              key0 === "body"
            )
          ) {
            validate39.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate39.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate39.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate39.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("event" !== data1) {
                validate39.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "event" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.eventId !== undefined) {
                  let data3 = data.eventId;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate39.errors = [
                          {
                            instancePath: instancePath + "/eventId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate39.errors = [
                            {
                              instancePath: instancePath + "/eventId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate39.errors = [
                              {
                                instancePath: instancePath + "/eventId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate39.errors = [
                        {
                          instancePath: instancePath + "/eventId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.sequence !== undefined) {
                    let data4 = data.sequence;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (
                      !(
                        typeof data4 == "number" &&
                        !(data4 % 1) &&
                        !isNaN(data4) &&
                        isFinite(data4)
                      )
                    ) {
                      validate39.errors = [
                        {
                          instancePath: instancePath + "/sequence",
                          schemaPath: "#/$defs/Counter/type",
                          keyword: "type",
                          params: { type: "integer" },
                          message: "must be integer",
                        },
                      ];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data4 == "number" && isFinite(data4)) {
                        if (data4 > 9007199254740991 || isNaN(data4)) {
                          validate39.errors = [
                            {
                              instancePath: instancePath + "/sequence",
                              schemaPath: "#/$defs/Counter/maximum",
                              keyword: "maximum",
                              params: {
                                comparison: "<=",
                                limit: 9007199254740991,
                              },
                              message: "must be <= 9007199254740991",
                            },
                          ];
                          return false;
                        } else {
                          if (data4 < 0 || isNaN(data4)) {
                            validate39.errors = [
                              {
                                instancePath: instancePath + "/sequence",
                                schemaPath: "#/$defs/Counter/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.commandId !== undefined) {
                      let data5 = data.commandId;
                      const _errs13 = errors;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (typeof data5 === "string") {
                          if (func1(data5) > 128) {
                            validate39.errors = [
                              {
                                instancePath: instancePath + "/commandId",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (func1(data5) < 1) {
                              validate39.errors = [
                                {
                                  instancePath: instancePath + "/commandId",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (!pattern4.test(data5)) {
                                validate39.errors = [
                                  {
                                    instancePath: instancePath + "/commandId",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate39.errors = [
                            {
                              instancePath: instancePath + "/commandId",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.generation !== undefined) {
                        let data6 = data.generation;
                        const _errs16 = errors;
                        const _errs17 = errors;
                        if (errors === _errs17) {
                          if (typeof data6 === "string") {
                            if (func1(data6) > 128) {
                              validate39.errors = [
                                {
                                  instancePath: instancePath + "/generation",
                                  schemaPath: "#/$defs/Id/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (func1(data6) < 1) {
                                validate39.errors = [
                                  {
                                    instancePath: instancePath + "/generation",
                                    schemaPath: "#/$defs/Id/minLength",
                                    keyword: "minLength",
                                    params: { limit: 1 },
                                    message:
                                      "must NOT have fewer than 1 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (!pattern4.test(data6)) {
                                  validate39.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/generation",
                                      schemaPath: "#/$defs/Id/pattern",
                                      keyword: "pattern",
                                      params: {
                                        pattern:
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                      },
                                      message:
                                        'must match pattern "' +
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                        '"',
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          } else {
                            validate39.errors = [
                              {
                                instancePath: instancePath + "/generation",
                                schemaPath: "#/$defs/Id/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs16 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.body !== undefined) {
                          const _errs19 = errors;
                          if (
                            !validate41(data.body, {
                              instancePath: instancePath + "/body",
                              parentData: data,
                              parentDataProperty: "body",
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate41.errors
                                : vErrors.concat(validate41.errors);
                            errors = vErrors.length;
                          }
                          var valid0 = _errs19 === errors;
                        } else {
                          var valid0 = true;
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate39.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate39.errors = vErrors;
  return errors === 0;
}
validate39.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema99 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "session",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    revision: {
      $ref: "#/$defs/Counter",
      description: "Monotonic CAS revision of this product record.",
    },
    lastSequence: {
      $ref: "#/$defs/Counter",
      description:
        "Highest committed stable-event sequence at this session revision.",
    },
    binding: {
      $ref: "#/$defs/Binding",
      description: "Exact provider incarnation and native context identity.",
    },
    capabilities: {
      $ref: "#/$defs/Capabilities",
      description:
        "Capabilities bound to this exact provider/configuration/account incarnation.",
    },
    status: {
      type: "string",
      enum: ["active", "retired"],
      description:
        "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "revision",
    "lastSequence",
    "binding",
    "capabilities",
    "status",
  ],
  additionalProperties: false,
  description:
    "Logical session state and stable event watermark committed at one revision.",
};
const schema102 = {
  type: "object",
  properties: {
    provider: { $ref: "#/$defs/Id", description: "Provider adapter identity." },
    providerVersion: {
      $ref: "#/$defs/Id",
      description: "Pinned native provider implementation version.",
    },
    adapterVersion: {
      $ref: "#/$defs/Id",
      description:
        "Adapter implementation version used for capability verification.",
    },
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    accountRef: {
      $ref: "#/$defs/Id",
      description:
        "Opaque account reference; no token, key or account-directory contents.",
    },
    nativeSessionId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned context session identifier; history alone cannot recreate it.",
    },
    config: {
      $ref: "#/$defs/ConfigRef",
      description: "Exact immutable configuration identity and revision.",
    },
    nativeRunId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned model-turn/run identifier, required when the provider exposes it.",
    },
    nativeRequestId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction.",
    },
  },
  required: [
    "provider",
    "providerVersion",
    "adapterVersion",
    "generation",
    "accountRef",
    "nativeSessionId",
    "config",
  ],
  additionalProperties: false,
  description:
    "Provider context identity. Version, configuration, account and generation bind every capability and callback.",
};
const schema109 = {
  type: "object",
  properties: {
    id: {
      $ref: "#/$defs/Id",
      description:
        "Configuration identifier; resolve credentials outside the wire.",
    },
    revision: {
      $ref: "#/$defs/Id",
      description:
        "Immutable configuration revision; changing it invalidates prior capability evidence.",
    },
  },
  required: ["id", "revision"],
  additionalProperties: false,
  description:
    "Immutable configuration identity and revision; contains no credentials.",
};
function validate51(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate51.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.id === undefined && (missing0 = "id")) ||
        (data.revision === undefined && (missing0 = "revision"))
      ) {
        validate51.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "id" || key0 === "revision")) {
            validate51.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.id !== undefined) {
            let data0 = data.id;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate51.errors = [
                    {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate51.errors = [
                      {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate51.errors = [
                        {
                          instancePath: instancePath + "/id",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate51.errors = [
                  {
                    instancePath: instancePath + "/id",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.revision !== undefined) {
              let data1 = data.revision;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate51.errors = [
                      {
                        instancePath: instancePath + "/revision",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate51.errors = [
                        {
                          instancePath: instancePath + "/revision",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate51.errors = [
                          {
                            instancePath: instancePath + "/revision",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate51.errors = [
                    {
                      instancePath: instancePath + "/revision",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate51.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate51.errors = vErrors;
  return errors === 0;
}
validate51.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate50(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate50.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.provider === undefined && (missing0 = "provider")) ||
        (data.providerVersion === undefined &&
          (missing0 = "providerVersion")) ||
        (data.adapterVersion === undefined && (missing0 = "adapterVersion")) ||
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.accountRef === undefined && (missing0 = "accountRef")) ||
        (data.nativeSessionId === undefined &&
          (missing0 = "nativeSessionId")) ||
        (data.config === undefined && (missing0 = "config"))
      ) {
        validate50.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema102.properties, key0)) {
            validate50.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.provider !== undefined) {
            let data0 = data.provider;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate50.errors = [
                    {
                      instancePath: instancePath + "/provider",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate50.errors = [
                      {
                        instancePath: instancePath + "/provider",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate50.errors = [
                        {
                          instancePath: instancePath + "/provider",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate50.errors = [
                  {
                    instancePath: instancePath + "/provider",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.providerVersion !== undefined) {
              let data1 = data.providerVersion;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate50.errors = [
                      {
                        instancePath: instancePath + "/providerVersion",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate50.errors = [
                        {
                          instancePath: instancePath + "/providerVersion",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate50.errors = [
                          {
                            instancePath: instancePath + "/providerVersion",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate50.errors = [
                    {
                      instancePath: instancePath + "/providerVersion",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.adapterVersion !== undefined) {
                let data2 = data.adapterVersion;
                const _errs8 = errors;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate50.errors = [
                        {
                          instancePath: instancePath + "/adapterVersion",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate50.errors = [
                          {
                            instancePath: instancePath + "/adapterVersion",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate50.errors = [
                            {
                              instancePath: instancePath + "/adapterVersion",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate50.errors = [
                      {
                        instancePath: instancePath + "/adapterVersion",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs8 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.generation !== undefined) {
                  let data3 = data.generation;
                  const _errs11 = errors;
                  const _errs12 = errors;
                  if (errors === _errs12) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate50.errors = [
                          {
                            instancePath: instancePath + "/generation",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate50.errors = [
                            {
                              instancePath: instancePath + "/generation",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate50.errors = [
                              {
                                instancePath: instancePath + "/generation",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate50.errors = [
                        {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs11 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.accountRef !== undefined) {
                    let data4 = data.accountRef;
                    const _errs14 = errors;
                    const _errs15 = errors;
                    if (errors === _errs15) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate50.errors = [
                            {
                              instancePath: instancePath + "/accountRef",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate50.errors = [
                              {
                                instancePath: instancePath + "/accountRef",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate50.errors = [
                                {
                                  instancePath: instancePath + "/accountRef",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate50.errors = [
                          {
                            instancePath: instancePath + "/accountRef",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs14 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.nativeSessionId !== undefined) {
                      let data5 = data.nativeSessionId;
                      const _errs17 = errors;
                      const _errs18 = errors;
                      if (errors === _errs18) {
                        if (typeof data5 === "string") {
                          if (func1(data5) > 128) {
                            validate50.errors = [
                              {
                                instancePath: instancePath + "/nativeSessionId",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (func1(data5) < 1) {
                              validate50.errors = [
                                {
                                  instancePath:
                                    instancePath + "/nativeSessionId",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (!pattern4.test(data5)) {
                                validate50.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/nativeSessionId",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate50.errors = [
                            {
                              instancePath: instancePath + "/nativeSessionId",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs17 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.config !== undefined) {
                        const _errs20 = errors;
                        if (
                          !validate51(data.config, {
                            instancePath: instancePath + "/config",
                            parentData: data,
                            parentDataProperty: "config",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate51.errors
                              : vErrors.concat(validate51.errors);
                          errors = vErrors.length;
                        }
                        var valid0 = _errs20 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.nativeRunId !== undefined) {
                          let data7 = data.nativeRunId;
                          const _errs21 = errors;
                          const _errs22 = errors;
                          if (errors === _errs22) {
                            if (typeof data7 === "string") {
                              if (func1(data7) > 128) {
                                validate50.errors = [
                                  {
                                    instancePath: instancePath + "/nativeRunId",
                                    schemaPath: "#/$defs/Id/maxLength",
                                    keyword: "maxLength",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (func1(data7) < 1) {
                                  validate50.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/nativeRunId",
                                      schemaPath: "#/$defs/Id/minLength",
                                      keyword: "minLength",
                                      params: { limit: 1 },
                                      message:
                                        "must NOT have fewer than 1 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (!pattern4.test(data7)) {
                                    validate50.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/nativeRunId",
                                        schemaPath: "#/$defs/Id/pattern",
                                        keyword: "pattern",
                                        params: {
                                          pattern:
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                        },
                                        message:
                                          'must match pattern "' +
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                          '"',
                                      },
                                    ];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate50.errors = [
                                {
                                  instancePath: instancePath + "/nativeRunId",
                                  schemaPath: "#/$defs/Id/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs21 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.nativeRequestId !== undefined) {
                            let data8 = data.nativeRequestId;
                            const _errs24 = errors;
                            const _errs25 = errors;
                            if (errors === _errs25) {
                              if (typeof data8 === "string") {
                                if (func1(data8) > 128) {
                                  validate50.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/nativeRequestId",
                                      schemaPath: "#/$defs/Id/maxLength",
                                      keyword: "maxLength",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (func1(data8) < 1) {
                                    validate50.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/nativeRequestId",
                                        schemaPath: "#/$defs/Id/minLength",
                                        keyword: "minLength",
                                        params: { limit: 1 },
                                        message:
                                          "must NOT have fewer than 1 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (!pattern4.test(data8)) {
                                      validate50.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/nativeRequestId",
                                          schemaPath: "#/$defs/Id/pattern",
                                          keyword: "pattern",
                                          params: {
                                            pattern:
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                          },
                                          message:
                                            'must match pattern "' +
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                            '"',
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate50.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/nativeRequestId",
                                    schemaPath: "#/$defs/Id/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs24 === errors;
                          } else {
                            var valid0 = true;
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate50.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate50.errors = vErrors;
  return errors === 0;
}
validate50.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema114 = {
  type: "object",
  properties: {
    continuation: {
      type: "string",
      enum: ["same_process", "across_processes", "unsupported", "unknown"],
      description:
        "same_process resumes only a live context; across_processes requires verified provider restoration; unsupported/unknown cannot resume.",
    },
    cancellation: {
      type: "string",
      enum: ["request_only", "terminal_acknowledged", "unsupported", "unknown"],
      description:
        "request_only confirms sending; terminal_acknowledged requires native terminal evidence; unsupported/unknown cannot promise cancellation.",
    },
    tools: {
      type: "string",
      enum: ["host_mediated", "provider_managed", "disabled", "unknown"],
      description:
        "host_mediated still requires containment evidence; provider_managed is not controlled execution; disabled/unknown cannot enable tools.",
    },
    steer: {
      $ref: "#/$defs/CapabilityState",
      description: "Whether an active native run accepts targeted steering.",
    },
    fork: {
      $ref: "#/$defs/CapabilityState",
      description: "Whether the provider supports an explicit context fork.",
    },
    subagent: {
      $ref: "#/$defs/CapabilityState",
      description:
        "Whether the provider supports child agents; not execution authorization.",
    },
    terminal: {
      $ref: "#/$defs/CapabilityState",
      description:
        "Whether the provider exposes a terminal facility; not the command terminal state.",
    },
    structuredQuestion: {
      $ref: "#/$defs/CapabilityState",
      description:
        "Whether a native structured callback can be represented and answered.",
    },
    multimodal: {
      $ref: "#/$defs/CapabilityState",
      description:
        "Whether provider-specific multimodal input is available through an adapter extension.",
    },
    queue: {
      $ref: "#/$defs/CapabilityState",
      description:
        "Whether additional prompts may be queued while a run is active.",
    },
  },
  required: [
    "continuation",
    "cancellation",
    "tools",
    "steer",
    "fork",
    "subagent",
    "terminal",
    "structuredQuestion",
    "multimodal",
    "queue",
  ],
  additionalProperties: false,
  description:
    "Capabilities established for one exact provider binding, never execution authorization.",
};
const schema115 = {
  type: "string",
  enum: ["supported", "unsupported", "unknown"],
  description:
    "Only supported enables an operation; unknown and unsupported fail closed.",
};
function validate54(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate54.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.continuation === undefined && (missing0 = "continuation")) ||
        (data.cancellation === undefined && (missing0 = "cancellation")) ||
        (data.tools === undefined && (missing0 = "tools")) ||
        (data.steer === undefined && (missing0 = "steer")) ||
        (data.fork === undefined && (missing0 = "fork")) ||
        (data.subagent === undefined && (missing0 = "subagent")) ||
        (data.terminal === undefined && (missing0 = "terminal")) ||
        (data.structuredQuestion === undefined &&
          (missing0 = "structuredQuestion")) ||
        (data.multimodal === undefined && (missing0 = "multimodal")) ||
        (data.queue === undefined && (missing0 = "queue"))
      ) {
        validate54.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema114.properties, key0)) {
            validate54.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.continuation !== undefined) {
            let data0 = data.continuation;
            const _errs2 = errors;
            if (typeof data0 !== "string") {
              validate54.errors = [
                {
                  instancePath: instancePath + "/continuation",
                  schemaPath: "#/properties/continuation/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                },
              ];
              return false;
            }
            if (
              !(
                data0 === "same_process" ||
                data0 === "across_processes" ||
                data0 === "unsupported" ||
                data0 === "unknown"
              )
            ) {
              validate54.errors = [
                {
                  instancePath: instancePath + "/continuation",
                  schemaPath: "#/properties/continuation/enum",
                  keyword: "enum",
                  params: {
                    allowedValues: schema114.properties.continuation.enum,
                  },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.cancellation !== undefined) {
              let data1 = data.cancellation;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate54.errors = [
                  {
                    instancePath: instancePath + "/cancellation",
                    schemaPath: "#/properties/cancellation/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if (
                !(
                  data1 === "request_only" ||
                  data1 === "terminal_acknowledged" ||
                  data1 === "unsupported" ||
                  data1 === "unknown"
                )
              ) {
                validate54.errors = [
                  {
                    instancePath: instancePath + "/cancellation",
                    schemaPath: "#/properties/cancellation/enum",
                    keyword: "enum",
                    params: {
                      allowedValues: schema114.properties.cancellation.enum,
                    },
                    message: "must be equal to one of the allowed values",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.tools !== undefined) {
                let data2 = data.tools;
                const _errs6 = errors;
                if (typeof data2 !== "string") {
                  validate54.errors = [
                    {
                      instancePath: instancePath + "/tools",
                      schemaPath: "#/properties/tools/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
                if (
                  !(
                    data2 === "host_mediated" ||
                    data2 === "provider_managed" ||
                    data2 === "disabled" ||
                    data2 === "unknown"
                  )
                ) {
                  validate54.errors = [
                    {
                      instancePath: instancePath + "/tools",
                      schemaPath: "#/properties/tools/enum",
                      keyword: "enum",
                      params: {
                        allowedValues: schema114.properties.tools.enum,
                      },
                      message: "must be equal to one of the allowed values",
                    },
                  ];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.steer !== undefined) {
                  let data3 = data.steer;
                  const _errs8 = errors;
                  if (typeof data3 !== "string") {
                    validate54.errors = [
                      {
                        instancePath: instancePath + "/steer",
                        schemaPath: "#/$defs/CapabilityState/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  if (
                    !(
                      data3 === "supported" ||
                      data3 === "unsupported" ||
                      data3 === "unknown"
                    )
                  ) {
                    validate54.errors = [
                      {
                        instancePath: instancePath + "/steer",
                        schemaPath: "#/$defs/CapabilityState/enum",
                        keyword: "enum",
                        params: { allowedValues: schema115.enum },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.fork !== undefined) {
                    let data4 = data.fork;
                    const _errs11 = errors;
                    if (typeof data4 !== "string") {
                      validate54.errors = [
                        {
                          instancePath: instancePath + "/fork",
                          schemaPath: "#/$defs/CapabilityState/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "supported" ||
                        data4 === "unsupported" ||
                        data4 === "unknown"
                      )
                    ) {
                      validate54.errors = [
                        {
                          instancePath: instancePath + "/fork",
                          schemaPath: "#/$defs/CapabilityState/enum",
                          keyword: "enum",
                          params: { allowedValues: schema115.enum },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid0 = _errs11 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.subagent !== undefined) {
                      let data5 = data.subagent;
                      const _errs14 = errors;
                      if (typeof data5 !== "string") {
                        validate54.errors = [
                          {
                            instancePath: instancePath + "/subagent",
                            schemaPath: "#/$defs/CapabilityState/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                      if (
                        !(
                          data5 === "supported" ||
                          data5 === "unsupported" ||
                          data5 === "unknown"
                        )
                      ) {
                        validate54.errors = [
                          {
                            instancePath: instancePath + "/subagent",
                            schemaPath: "#/$defs/CapabilityState/enum",
                            keyword: "enum",
                            params: { allowedValues: schema115.enum },
                            message:
                              "must be equal to one of the allowed values",
                          },
                        ];
                        return false;
                      }
                      var valid0 = _errs14 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.terminal !== undefined) {
                        let data6 = data.terminal;
                        const _errs17 = errors;
                        if (typeof data6 !== "string") {
                          validate54.errors = [
                            {
                              instancePath: instancePath + "/terminal",
                              schemaPath: "#/$defs/CapabilityState/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        if (
                          !(
                            data6 === "supported" ||
                            data6 === "unsupported" ||
                            data6 === "unknown"
                          )
                        ) {
                          validate54.errors = [
                            {
                              instancePath: instancePath + "/terminal",
                              schemaPath: "#/$defs/CapabilityState/enum",
                              keyword: "enum",
                              params: { allowedValues: schema115.enum },
                              message:
                                "must be equal to one of the allowed values",
                            },
                          ];
                          return false;
                        }
                        var valid0 = _errs17 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.structuredQuestion !== undefined) {
                          let data7 = data.structuredQuestion;
                          const _errs20 = errors;
                          if (typeof data7 !== "string") {
                            validate54.errors = [
                              {
                                instancePath:
                                  instancePath + "/structuredQuestion",
                                schemaPath: "#/$defs/CapabilityState/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          if (
                            !(
                              data7 === "supported" ||
                              data7 === "unsupported" ||
                              data7 === "unknown"
                            )
                          ) {
                            validate54.errors = [
                              {
                                instancePath:
                                  instancePath + "/structuredQuestion",
                                schemaPath: "#/$defs/CapabilityState/enum",
                                keyword: "enum",
                                params: { allowedValues: schema115.enum },
                                message:
                                  "must be equal to one of the allowed values",
                              },
                            ];
                            return false;
                          }
                          var valid0 = _errs20 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.multimodal !== undefined) {
                            let data8 = data.multimodal;
                            const _errs23 = errors;
                            if (typeof data8 !== "string") {
                              validate54.errors = [
                                {
                                  instancePath: instancePath + "/multimodal",
                                  schemaPath: "#/$defs/CapabilityState/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                            if (
                              !(
                                data8 === "supported" ||
                                data8 === "unsupported" ||
                                data8 === "unknown"
                              )
                            ) {
                              validate54.errors = [
                                {
                                  instancePath: instancePath + "/multimodal",
                                  schemaPath: "#/$defs/CapabilityState/enum",
                                  keyword: "enum",
                                  params: { allowedValues: schema115.enum },
                                  message:
                                    "must be equal to one of the allowed values",
                                },
                              ];
                              return false;
                            }
                            var valid0 = _errs23 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.queue !== undefined) {
                              let data9 = data.queue;
                              const _errs26 = errors;
                              if (typeof data9 !== "string") {
                                validate54.errors = [
                                  {
                                    instancePath: instancePath + "/queue",
                                    schemaPath: "#/$defs/CapabilityState/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                              if (
                                !(
                                  data9 === "supported" ||
                                  data9 === "unsupported" ||
                                  data9 === "unknown"
                                )
                              ) {
                                validate54.errors = [
                                  {
                                    instancePath: instancePath + "/queue",
                                    schemaPath: "#/$defs/CapabilityState/enum",
                                    keyword: "enum",
                                    params: { allowedValues: schema115.enum },
                                    message:
                                      "must be equal to one of the allowed values",
                                  },
                                ];
                                return false;
                              }
                              var valid0 = _errs26 === errors;
                            } else {
                              var valid0 = true;
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate54.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate54.errors = vErrors;
  return errors === 0;
}
validate54.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate48(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate48.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.revision === undefined && (missing0 = "revision")) ||
        (data.lastSequence === undefined && (missing0 = "lastSequence")) ||
        (data.binding === undefined && (missing0 = "binding")) ||
        (data.capabilities === undefined && (missing0 = "capabilities")) ||
        (data.status === undefined && (missing0 = "status"))
      ) {
        validate48.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "namespace" ||
              key0 === "revision" ||
              key0 === "lastSequence" ||
              key0 === "binding" ||
              key0 === "capabilities" ||
              key0 === "status"
            )
          ) {
            validate48.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate48.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate48.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate48.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("session" !== data1) {
                validate48.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "session" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.revision !== undefined) {
                  let data3 = data.revision;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (
                    !(
                      typeof data3 == "number" &&
                      !(data3 % 1) &&
                      !isNaN(data3) &&
                      isFinite(data3)
                    )
                  ) {
                    validate48.errors = [
                      {
                        instancePath: instancePath + "/revision",
                        schemaPath: "#/$defs/Counter/type",
                        keyword: "type",
                        params: { type: "integer" },
                        message: "must be integer",
                      },
                    ];
                    return false;
                  }
                  if (errors === _errs8) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 9007199254740991 || isNaN(data3)) {
                        validate48.errors = [
                          {
                            instancePath: instancePath + "/revision",
                            schemaPath: "#/$defs/Counter/maximum",
                            keyword: "maximum",
                            params: {
                              comparison: "<=",
                              limit: 9007199254740991,
                            },
                            message: "must be <= 9007199254740991",
                          },
                        ];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate48.errors = [
                            {
                              instancePath: instancePath + "/revision",
                              schemaPath: "#/$defs/Counter/minimum",
                              keyword: "minimum",
                              params: { comparison: ">=", limit: 0 },
                              message: "must be >= 0",
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.lastSequence !== undefined) {
                    let data4 = data.lastSequence;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (
                      !(
                        typeof data4 == "number" &&
                        !(data4 % 1) &&
                        !isNaN(data4) &&
                        isFinite(data4)
                      )
                    ) {
                      validate48.errors = [
                        {
                          instancePath: instancePath + "/lastSequence",
                          schemaPath: "#/$defs/Counter/type",
                          keyword: "type",
                          params: { type: "integer" },
                          message: "must be integer",
                        },
                      ];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data4 == "number" && isFinite(data4)) {
                        if (data4 > 9007199254740991 || isNaN(data4)) {
                          validate48.errors = [
                            {
                              instancePath: instancePath + "/lastSequence",
                              schemaPath: "#/$defs/Counter/maximum",
                              keyword: "maximum",
                              params: {
                                comparison: "<=",
                                limit: 9007199254740991,
                              },
                              message: "must be <= 9007199254740991",
                            },
                          ];
                          return false;
                        } else {
                          if (data4 < 0 || isNaN(data4)) {
                            validate48.errors = [
                              {
                                instancePath: instancePath + "/lastSequence",
                                schemaPath: "#/$defs/Counter/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.binding !== undefined) {
                      const _errs13 = errors;
                      if (
                        !validate50(data.binding, {
                          instancePath: instancePath + "/binding",
                          parentData: data,
                          parentDataProperty: "binding",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate50.errors
                            : vErrors.concat(validate50.errors);
                        errors = vErrors.length;
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.capabilities !== undefined) {
                        const _errs14 = errors;
                        if (
                          !validate54(data.capabilities, {
                            instancePath: instancePath + "/capabilities",
                            parentData: data,
                            parentDataProperty: "capabilities",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate54.errors
                              : vErrors.concat(validate54.errors);
                          errors = vErrors.length;
                        }
                        var valid0 = _errs14 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.status !== undefined) {
                          let data7 = data.status;
                          const _errs15 = errors;
                          if (typeof data7 !== "string") {
                            validate48.errors = [
                              {
                                instancePath: instancePath + "/status",
                                schemaPath: "#/properties/status/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          if (!(data7 === "active" || data7 === "retired")) {
                            validate48.errors = [
                              {
                                instancePath: instancePath + "/status",
                                schemaPath: "#/properties/status/enum",
                                keyword: "enum",
                                params: {
                                  allowedValues:
                                    schema99.properties.status.enum,
                                },
                                message:
                                  "must be equal to one of the allowed values",
                              },
                            ];
                            return false;
                          }
                          var valid0 = _errs15 === errors;
                        } else {
                          var valid0 = true;
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate48.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate48.errors = vErrors;
  return errors === 0;
}
validate48.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema122 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "interaction",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    interactionId: {
      $ref: "#/$defs/Id",
      description: "Single-use interaction identity within the namespace.",
    },
    commandId: {
      $ref: "#/$defs/Id",
      description:
        "Client-generated idempotency key; reuse only with identical canonical content.",
    },
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    nativeRunId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned model-turn/run identifier, required when the provider exposes it.",
    },
    expiresAtMs: {
      $ref: "#/$defs/Counter",
      description:
        "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected.",
    },
    status: {
      type: "string",
      enum: ["pending", "answered", "expired", "unavailable"],
      description:
        "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
    },
    responseCommandId: {
      $ref: "#/$defs/Id",
      description:
        "Accepted response command which atomically consumed the interaction; present only when answered.",
    },
    callbackLifetime: {
      $ref: "#/$defs/CallbackLifetime",
      description:
        "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration.",
    },
    nativeCallbackId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned callback identifier, immutable and unique within a session generation; distinct from the parent dispatch request.",
    },
    request: {
      $ref: "#/$defs/InteractionRequest",
      description:
        "Immutable untrusted question payload retained for display; does not restore a lost native callback or grant approval.",
    },
    category: {
      type: "string",
      const: "question",
      description:
        "Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "interactionId",
    "commandId",
    "generation",
    "nativeCallbackId",
    "expiresAtMs",
    "status",
    "callbackLifetime",
    "request",
    "category",
  ],
  additionalProperties: false,
  description:
    "Single-use provider callback with immutable command/native correlation, expiry and lifetime.",
};
function validate57(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate57.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.interactionId === undefined && (missing0 = "interactionId")) ||
        (data.commandId === undefined && (missing0 = "commandId")) ||
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.nativeCallbackId === undefined &&
          (missing0 = "nativeCallbackId")) ||
        (data.expiresAtMs === undefined && (missing0 = "expiresAtMs")) ||
        (data.status === undefined && (missing0 = "status")) ||
        (data.callbackLifetime === undefined &&
          (missing0 = "callbackLifetime")) ||
        (data.request === undefined && (missing0 = "request")) ||
        (data.category === undefined && (missing0 = "category"))
      ) {
        validate57.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema122.properties, key0)) {
            validate57.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate57.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate57.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate57.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("interaction" !== data1) {
                validate57.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "interaction" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.interactionId !== undefined) {
                  let data3 = data.interactionId;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate57.errors = [
                          {
                            instancePath: instancePath + "/interactionId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate57.errors = [
                            {
                              instancePath: instancePath + "/interactionId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate57.errors = [
                              {
                                instancePath: instancePath + "/interactionId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate57.errors = [
                        {
                          instancePath: instancePath + "/interactionId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.commandId !== undefined) {
                    let data4 = data.commandId;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (errors === _errs11) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate57.errors = [
                            {
                              instancePath: instancePath + "/commandId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate57.errors = [
                              {
                                instancePath: instancePath + "/commandId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate57.errors = [
                                {
                                  instancePath: instancePath + "/commandId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate57.errors = [
                          {
                            instancePath: instancePath + "/commandId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.generation !== undefined) {
                      let data5 = data.generation;
                      const _errs13 = errors;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (typeof data5 === "string") {
                          if (func1(data5) > 128) {
                            validate57.errors = [
                              {
                                instancePath: instancePath + "/generation",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (func1(data5) < 1) {
                              validate57.errors = [
                                {
                                  instancePath: instancePath + "/generation",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (!pattern4.test(data5)) {
                                validate57.errors = [
                                  {
                                    instancePath: instancePath + "/generation",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate57.errors = [
                            {
                              instancePath: instancePath + "/generation",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.nativeRunId !== undefined) {
                        let data6 = data.nativeRunId;
                        const _errs16 = errors;
                        const _errs17 = errors;
                        if (errors === _errs17) {
                          if (typeof data6 === "string") {
                            if (func1(data6) > 128) {
                              validate57.errors = [
                                {
                                  instancePath: instancePath + "/nativeRunId",
                                  schemaPath: "#/$defs/Id/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (func1(data6) < 1) {
                                validate57.errors = [
                                  {
                                    instancePath: instancePath + "/nativeRunId",
                                    schemaPath: "#/$defs/Id/minLength",
                                    keyword: "minLength",
                                    params: { limit: 1 },
                                    message:
                                      "must NOT have fewer than 1 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (!pattern4.test(data6)) {
                                  validate57.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/nativeRunId",
                                      schemaPath: "#/$defs/Id/pattern",
                                      keyword: "pattern",
                                      params: {
                                        pattern:
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                      },
                                      message:
                                        'must match pattern "' +
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                        '"',
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          } else {
                            validate57.errors = [
                              {
                                instancePath: instancePath + "/nativeRunId",
                                schemaPath: "#/$defs/Id/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs16 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.expiresAtMs !== undefined) {
                          let data7 = data.expiresAtMs;
                          const _errs19 = errors;
                          const _errs20 = errors;
                          if (
                            !(
                              typeof data7 == "number" &&
                              !(data7 % 1) &&
                              !isNaN(data7) &&
                              isFinite(data7)
                            )
                          ) {
                            validate57.errors = [
                              {
                                instancePath: instancePath + "/expiresAtMs",
                                schemaPath: "#/$defs/Counter/type",
                                keyword: "type",
                                params: { type: "integer" },
                                message: "must be integer",
                              },
                            ];
                            return false;
                          }
                          if (errors === _errs20) {
                            if (typeof data7 == "number" && isFinite(data7)) {
                              if (data7 > 9007199254740991 || isNaN(data7)) {
                                validate57.errors = [
                                  {
                                    instancePath: instancePath + "/expiresAtMs",
                                    schemaPath: "#/$defs/Counter/maximum",
                                    keyword: "maximum",
                                    params: {
                                      comparison: "<=",
                                      limit: 9007199254740991,
                                    },
                                    message: "must be <= 9007199254740991",
                                  },
                                ];
                                return false;
                              } else {
                                if (data7 < 0 || isNaN(data7)) {
                                  validate57.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/expiresAtMs",
                                      schemaPath: "#/$defs/Counter/minimum",
                                      keyword: "minimum",
                                      params: { comparison: ">=", limit: 0 },
                                      message: "must be >= 0",
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          }
                          var valid0 = _errs19 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.status !== undefined) {
                            let data8 = data.status;
                            const _errs22 = errors;
                            if (typeof data8 !== "string") {
                              validate57.errors = [
                                {
                                  instancePath: instancePath + "/status",
                                  schemaPath: "#/properties/status/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                            if (
                              !(
                                data8 === "pending" ||
                                data8 === "answered" ||
                                data8 === "expired" ||
                                data8 === "unavailable"
                              )
                            ) {
                              validate57.errors = [
                                {
                                  instancePath: instancePath + "/status",
                                  schemaPath: "#/properties/status/enum",
                                  keyword: "enum",
                                  params: {
                                    allowedValues:
                                      schema122.properties.status.enum,
                                  },
                                  message:
                                    "must be equal to one of the allowed values",
                                },
                              ];
                              return false;
                            }
                            var valid0 = _errs22 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.responseCommandId !== undefined) {
                              let data9 = data.responseCommandId;
                              const _errs24 = errors;
                              const _errs25 = errors;
                              if (errors === _errs25) {
                                if (typeof data9 === "string") {
                                  if (func1(data9) > 128) {
                                    validate57.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/responseCommandId",
                                        schemaPath: "#/$defs/Id/maxLength",
                                        keyword: "maxLength",
                                        params: { limit: 128 },
                                        message:
                                          "must NOT have more than 128 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (func1(data9) < 1) {
                                      validate57.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/responseCommandId",
                                          schemaPath: "#/$defs/Id/minLength",
                                          keyword: "minLength",
                                          params: { limit: 1 },
                                          message:
                                            "must NOT have fewer than 1 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (!pattern4.test(data9)) {
                                        validate57.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/responseCommandId",
                                            schemaPath: "#/$defs/Id/pattern",
                                            keyword: "pattern",
                                            params: {
                                              pattern:
                                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                            },
                                            message:
                                              'must match pattern "' +
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                              '"',
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                  }
                                } else {
                                  validate57.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/responseCommandId",
                                      schemaPath: "#/$defs/Id/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid0 = _errs24 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.callbackLifetime !== undefined) {
                                let data10 = data.callbackLifetime;
                                const _errs27 = errors;
                                if (typeof data10 !== "string") {
                                  validate57.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/callbackLifetime",
                                      schemaPath:
                                        "#/$defs/CallbackLifetime/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                                if (
                                  !(
                                    data10 === "generation_bound" ||
                                    data10 === "provider_resumable"
                                  )
                                ) {
                                  validate57.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/callbackLifetime",
                                      schemaPath:
                                        "#/$defs/CallbackLifetime/enum",
                                      keyword: "enum",
                                      params: { allowedValues: schema84.enum },
                                      message:
                                        "must be equal to one of the allowed values",
                                    },
                                  ];
                                  return false;
                                }
                                var valid0 = _errs27 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.nativeCallbackId !== undefined) {
                                  let data11 = data.nativeCallbackId;
                                  const _errs30 = errors;
                                  const _errs31 = errors;
                                  if (errors === _errs31) {
                                    if (typeof data11 === "string") {
                                      if (func1(data11) > 128) {
                                        validate57.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/nativeCallbackId",
                                            schemaPath: "#/$defs/Id/maxLength",
                                            keyword: "maxLength",
                                            params: { limit: 128 },
                                            message:
                                              "must NOT have more than 128 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (func1(data11) < 1) {
                                          validate57.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/nativeCallbackId",
                                              schemaPath:
                                                "#/$defs/Id/minLength",
                                              keyword: "minLength",
                                              params: { limit: 1 },
                                              message:
                                                "must NOT have fewer than 1 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (!pattern4.test(data11)) {
                                            validate57.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/nativeCallbackId",
                                                schemaPath:
                                                  "#/$defs/Id/pattern",
                                                keyword: "pattern",
                                                params: {
                                                  pattern:
                                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                                },
                                                message:
                                                  'must match pattern "' +
                                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                                  '"',
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                      }
                                    } else {
                                      validate57.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/nativeCallbackId",
                                          schemaPath: "#/$defs/Id/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid0 = _errs30 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.request !== undefined) {
                                    let data12 = data.request;
                                    const _errs33 = errors;
                                    const _errs34 = errors;
                                    if (errors === _errs34) {
                                      if (
                                        data12 &&
                                        typeof data12 == "object" &&
                                        !Array.isArray(data12)
                                      ) {
                                      } else {
                                        validate57.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/request",
                                            schemaPath:
                                              "#/$defs/InteractionRequest/type",
                                            keyword: "type",
                                            params: { type: "object" },
                                            message: "must be object",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                    var valid0 = _errs33 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.category !== undefined) {
                                      let data13 = data.category;
                                      const _errs37 = errors;
                                      if (typeof data13 !== "string") {
                                        validate57.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/category",
                                            schemaPath:
                                              "#/properties/category/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                      if ("question" !== data13) {
                                        validate57.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/category",
                                            schemaPath:
                                              "#/properties/category/const",
                                            keyword: "const",
                                            params: {
                                              allowedValue: "question",
                                            },
                                            message:
                                              "must be equal to constant",
                                          },
                                        ];
                                        return false;
                                      }
                                      var valid0 = _errs37 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate57.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate57.errors = vErrors;
  return errors === 0;
}
validate57.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema132 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "delivery",
      description: "Closed product record discriminator.",
    },
    namespace: {
      $ref: "#/$defs/Namespace",
      description:
        "Trusted storage isolation scope; not copied from model or action content.",
    },
    operationId: {
      $ref: "#/$defs/Id",
      description:
        "Stable receiver idempotency key for this delivery; cannot be rebound to another event/target.",
    },
    eventId: {
      $ref: "#/$defs/Id",
      description: "Stable unique event identifier within the namespace.",
    },
    target: {
      $ref: "#/$defs/Id",
      description: "Opaque reliable-delivery destination identifier.",
    },
    contentHash: {
      type: "string",
      pattern: "^[a-f0-9]{64}$",
      description:
        "SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination.",
    },
    retry: {
      type: "string",
      enum: ["receiver_idempotent", "reconcile_first", "never"],
      description:
        "Explicit retry discipline; uncertainty never authorizes blind resubmission.",
    },
    status: {
      type: "string",
      enum: ["pending", "delivered", "reconciliation_required"],
      description:
        "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss.",
    },
    attempts: {
      $ref: "#/$defs/Counter",
      description: "Monotonic count of delivery attempts.",
    },
    nextAttemptAtMs: {
      $ref: "#/$defs/Counter",
      description: "UTC epoch-millisecond earliest eligible retry time.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "namespace",
    "operationId",
    "eventId",
    "target",
    "contentHash",
    "retry",
    "status",
    "attempts",
    "nextAttemptAtMs",
  ],
  additionalProperties: false,
  description:
    "Reliable cross-service outbox record bound to an immutable event and target.",
};
function validate60(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate60.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.namespace === undefined && (missing0 = "namespace")) ||
        (data.operationId === undefined && (missing0 = "operationId")) ||
        (data.eventId === undefined && (missing0 = "eventId")) ||
        (data.target === undefined && (missing0 = "target")) ||
        (data.contentHash === undefined && (missing0 = "contentHash")) ||
        (data.retry === undefined && (missing0 = "retry")) ||
        (data.status === undefined && (missing0 = "status")) ||
        (data.attempts === undefined && (missing0 = "attempts")) ||
        (data.nextAttemptAtMs === undefined && (missing0 = "nextAttemptAtMs"))
      ) {
        validate60.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema132.properties, key0)) {
            validate60.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate60.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate60.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate60.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("delivery" !== data1) {
                validate60.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "delivery" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.namespace !== undefined) {
                const _errs6 = errors;
                if (
                  !validate28(data.namespace, {
                    instancePath: instancePath + "/namespace",
                    parentData: data,
                    parentDataProperty: "namespace",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate28.errors
                      : vErrors.concat(validate28.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.operationId !== undefined) {
                  let data3 = data.operationId;
                  const _errs7 = errors;
                  const _errs8 = errors;
                  if (errors === _errs8) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate60.errors = [
                          {
                            instancePath: instancePath + "/operationId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate60.errors = [
                            {
                              instancePath: instancePath + "/operationId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/operationId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate60.errors = [
                        {
                          instancePath: instancePath + "/operationId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.eventId !== undefined) {
                    let data4 = data.eventId;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (errors === _errs11) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate60.errors = [
                            {
                              instancePath: instancePath + "/eventId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/eventId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate60.errors = [
                                {
                                  instancePath: instancePath + "/eventId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate60.errors = [
                          {
                            instancePath: instancePath + "/eventId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.target !== undefined) {
                      let data5 = data.target;
                      const _errs13 = errors;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (typeof data5 === "string") {
                          if (func1(data5) > 128) {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/target",
                                schemaPath: "#/$defs/Id/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (func1(data5) < 1) {
                              validate60.errors = [
                                {
                                  instancePath: instancePath + "/target",
                                  schemaPath: "#/$defs/Id/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (!pattern4.test(data5)) {
                                validate60.errors = [
                                  {
                                    instancePath: instancePath + "/target",
                                    schemaPath: "#/$defs/Id/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern:
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                      '"',
                                  },
                                ];
                                return false;
                              }
                            }
                          }
                        } else {
                          validate60.errors = [
                            {
                              instancePath: instancePath + "/target",
                              schemaPath: "#/$defs/Id/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.contentHash !== undefined) {
                        let data6 = data.contentHash;
                        const _errs16 = errors;
                        if (errors === _errs16) {
                          if (typeof data6 === "string") {
                            if (!pattern19.test(data6)) {
                              validate60.errors = [
                                {
                                  instancePath: instancePath + "/contentHash",
                                  schemaPath:
                                    "#/properties/contentHash/pattern",
                                  keyword: "pattern",
                                  params: { pattern: "^[a-f0-9]{64}$" },
                                  message:
                                    'must match pattern "' +
                                    "^[a-f0-9]{64}$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          } else {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/contentHash",
                                schemaPath: "#/properties/contentHash/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs16 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.retry !== undefined) {
                          let data7 = data.retry;
                          const _errs18 = errors;
                          if (typeof data7 !== "string") {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/retry",
                                schemaPath: "#/properties/retry/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          if (
                            !(
                              data7 === "receiver_idempotent" ||
                              data7 === "reconcile_first" ||
                              data7 === "never"
                            )
                          ) {
                            validate60.errors = [
                              {
                                instancePath: instancePath + "/retry",
                                schemaPath: "#/properties/retry/enum",
                                keyword: "enum",
                                params: {
                                  allowedValues:
                                    schema132.properties.retry.enum,
                                },
                                message:
                                  "must be equal to one of the allowed values",
                              },
                            ];
                            return false;
                          }
                          var valid0 = _errs18 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.status !== undefined) {
                            let data8 = data.status;
                            const _errs20 = errors;
                            if (typeof data8 !== "string") {
                              validate60.errors = [
                                {
                                  instancePath: instancePath + "/status",
                                  schemaPath: "#/properties/status/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                            if (
                              !(
                                data8 === "pending" ||
                                data8 === "delivered" ||
                                data8 === "reconciliation_required"
                              )
                            ) {
                              validate60.errors = [
                                {
                                  instancePath: instancePath + "/status",
                                  schemaPath: "#/properties/status/enum",
                                  keyword: "enum",
                                  params: {
                                    allowedValues:
                                      schema132.properties.status.enum,
                                  },
                                  message:
                                    "must be equal to one of the allowed values",
                                },
                              ];
                              return false;
                            }
                            var valid0 = _errs20 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.attempts !== undefined) {
                              let data9 = data.attempts;
                              const _errs22 = errors;
                              const _errs23 = errors;
                              if (
                                !(
                                  typeof data9 == "number" &&
                                  !(data9 % 1) &&
                                  !isNaN(data9) &&
                                  isFinite(data9)
                                )
                              ) {
                                validate60.errors = [
                                  {
                                    instancePath: instancePath + "/attempts",
                                    schemaPath: "#/$defs/Counter/type",
                                    keyword: "type",
                                    params: { type: "integer" },
                                    message: "must be integer",
                                  },
                                ];
                                return false;
                              }
                              if (errors === _errs23) {
                                if (
                                  typeof data9 == "number" &&
                                  isFinite(data9)
                                ) {
                                  if (
                                    data9 > 9007199254740991 ||
                                    isNaN(data9)
                                  ) {
                                    validate60.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/attempts",
                                        schemaPath: "#/$defs/Counter/maximum",
                                        keyword: "maximum",
                                        params: {
                                          comparison: "<=",
                                          limit: 9007199254740991,
                                        },
                                        message: "must be <= 9007199254740991",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (data9 < 0 || isNaN(data9)) {
                                      validate60.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/attempts",
                                          schemaPath: "#/$defs/Counter/minimum",
                                          keyword: "minimum",
                                          params: {
                                            comparison: ">=",
                                            limit: 0,
                                          },
                                          message: "must be >= 0",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                }
                              }
                              var valid0 = _errs22 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.nextAttemptAtMs !== undefined) {
                                let data10 = data.nextAttemptAtMs;
                                const _errs25 = errors;
                                const _errs26 = errors;
                                if (
                                  !(
                                    typeof data10 == "number" &&
                                    !(data10 % 1) &&
                                    !isNaN(data10) &&
                                    isFinite(data10)
                                  )
                                ) {
                                  validate60.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/nextAttemptAtMs",
                                      schemaPath: "#/$defs/Counter/type",
                                      keyword: "type",
                                      params: { type: "integer" },
                                      message: "must be integer",
                                    },
                                  ];
                                  return false;
                                }
                                if (errors === _errs26) {
                                  if (
                                    typeof data10 == "number" &&
                                    isFinite(data10)
                                  ) {
                                    if (
                                      data10 > 9007199254740991 ||
                                      isNaN(data10)
                                    ) {
                                      validate60.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/nextAttemptAtMs",
                                          schemaPath: "#/$defs/Counter/maximum",
                                          keyword: "maximum",
                                          params: {
                                            comparison: "<=",
                                            limit: 9007199254740991,
                                          },
                                          message:
                                            "must be <= 9007199254740991",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (data10 < 0 || isNaN(data10)) {
                                        validate60.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/nextAttemptAtMs",
                                            schemaPath:
                                              "#/$defs/Counter/minimum",
                                            keyword: "minimum",
                                            params: {
                                              comparison: ">=",
                                              limit: 0,
                                            },
                                            message: "must be >= 0",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                  }
                                }
                                var valid0 = _errs25 === errors;
                              } else {
                                var valid0 = true;
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate60.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate60.errors = vErrors;
  return errors === 0;
}
validate60.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema138 = {
  type: "object",
  properties: {
    schemaVersion: {
      type: "integer",
      const: 2,
      description:
        "Exact product wire version; V1 is rejected without migration or fallback.",
    },
    kind: {
      type: "string",
      const: "surfaceAction",
      description: "Closed product record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Logical session identifier, never reusable after retirement.",
    },
    commandId: {
      $ref: "#/$defs/Id",
      description:
        "Client-generated idempotency key; reuse only with identical canonical content.",
    },
    surfaceInstanceId: {
      $ref: "#/$defs/Id",
      description:
        "Fresh product identity for each surface creation; deletion permanently invalidates old actions.",
    },
    surfaceRevision: {
      $ref: "#/$defs/Counter",
      description:
        "Exact current surface revision required to accept this action.",
    },
    interactionId: {
      $ref: "#/$defs/Id",
      description: "Single-use interaction identity within the namespace.",
    },
    generation: {
      $ref: "#/$defs/Id",
      description:
        "Live provider incarnation token; rejects callbacks from previous incarnations.",
    },
    nativeRunId: {
      $ref: "#/$defs/Id",
      description:
        "Provider-owned model-turn/run identifier, required when the provider exposes it.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "sessionId",
    "commandId",
    "surfaceInstanceId",
    "surfaceRevision",
    "interactionId",
    "generation",
    "nativeRunId",
  ],
  additionalProperties: false,
  description:
    "Product metadata accompanying an unchanged upstream action; association does not grant permission.",
};
function validate64(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate64.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.commandId === undefined && (missing0 = "commandId")) ||
        (data.surfaceInstanceId === undefined &&
          (missing0 = "surfaceInstanceId")) ||
        (data.surfaceRevision === undefined &&
          (missing0 = "surfaceRevision")) ||
        (data.interactionId === undefined && (missing0 = "interactionId")) ||
        (data.generation === undefined && (missing0 = "generation")) ||
        (data.nativeRunId === undefined && (missing0 = "nativeRunId"))
      ) {
        validate64.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema138.properties, key0)) {
            validate64.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate64.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate64.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate64.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("surfaceAction" !== data1) {
                validate64.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "surfaceAction" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate64.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate64.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate64.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate64.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.commandId !== undefined) {
                  let data3 = data.commandId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate64.errors = [
                          {
                            instancePath: instancePath + "/commandId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate64.errors = [
                            {
                              instancePath: instancePath + "/commandId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate64.errors = [
                              {
                                instancePath: instancePath + "/commandId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate64.errors = [
                        {
                          instancePath: instancePath + "/commandId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.surfaceInstanceId !== undefined) {
                    let data4 = data.surfaceInstanceId;
                    const _errs12 = errors;
                    const _errs13 = errors;
                    if (errors === _errs13) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate64.errors = [
                            {
                              instancePath: instancePath + "/surfaceInstanceId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate64.errors = [
                              {
                                instancePath:
                                  instancePath + "/surfaceInstanceId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate64.errors = [
                                {
                                  instancePath:
                                    instancePath + "/surfaceInstanceId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate64.errors = [
                          {
                            instancePath: instancePath + "/surfaceInstanceId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.surfaceRevision !== undefined) {
                      let data5 = data.surfaceRevision;
                      const _errs15 = errors;
                      const _errs16 = errors;
                      if (
                        !(
                          typeof data5 == "number" &&
                          !(data5 % 1) &&
                          !isNaN(data5) &&
                          isFinite(data5)
                        )
                      ) {
                        validate64.errors = [
                          {
                            instancePath: instancePath + "/surfaceRevision",
                            schemaPath: "#/$defs/Counter/type",
                            keyword: "type",
                            params: { type: "integer" },
                            message: "must be integer",
                          },
                        ];
                        return false;
                      }
                      if (errors === _errs16) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate64.errors = [
                              {
                                instancePath: instancePath + "/surfaceRevision",
                                schemaPath: "#/$defs/Counter/maximum",
                                keyword: "maximum",
                                params: {
                                  comparison: "<=",
                                  limit: 9007199254740991,
                                },
                                message: "must be <= 9007199254740991",
                              },
                            ];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate64.errors = [
                                {
                                  instancePath:
                                    instancePath + "/surfaceRevision",
                                  schemaPath: "#/$defs/Counter/minimum",
                                  keyword: "minimum",
                                  params: { comparison: ">=", limit: 0 },
                                  message: "must be >= 0",
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      }
                      var valid0 = _errs15 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.interactionId !== undefined) {
                        let data6 = data.interactionId;
                        const _errs18 = errors;
                        const _errs19 = errors;
                        if (errors === _errs19) {
                          if (typeof data6 === "string") {
                            if (func1(data6) > 128) {
                              validate64.errors = [
                                {
                                  instancePath: instancePath + "/interactionId",
                                  schemaPath: "#/$defs/Id/maxLength",
                                  keyword: "maxLength",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 characters",
                                },
                              ];
                              return false;
                            } else {
                              if (func1(data6) < 1) {
                                validate64.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath: "#/$defs/Id/minLength",
                                    keyword: "minLength",
                                    params: { limit: 1 },
                                    message:
                                      "must NOT have fewer than 1 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (!pattern4.test(data6)) {
                                  validate64.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/interactionId",
                                      schemaPath: "#/$defs/Id/pattern",
                                      keyword: "pattern",
                                      params: {
                                        pattern:
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                      },
                                      message:
                                        'must match pattern "' +
                                        "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                        '"',
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                          } else {
                            validate64.errors = [
                              {
                                instancePath: instancePath + "/interactionId",
                                schemaPath: "#/$defs/Id/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs18 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.generation !== undefined) {
                          let data7 = data.generation;
                          const _errs21 = errors;
                          const _errs22 = errors;
                          if (errors === _errs22) {
                            if (typeof data7 === "string") {
                              if (func1(data7) > 128) {
                                validate64.errors = [
                                  {
                                    instancePath: instancePath + "/generation",
                                    schemaPath: "#/$defs/Id/maxLength",
                                    keyword: "maxLength",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (func1(data7) < 1) {
                                  validate64.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/generation",
                                      schemaPath: "#/$defs/Id/minLength",
                                      keyword: "minLength",
                                      params: { limit: 1 },
                                      message:
                                        "must NOT have fewer than 1 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (!pattern4.test(data7)) {
                                    validate64.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/generation",
                                        schemaPath: "#/$defs/Id/pattern",
                                        keyword: "pattern",
                                        params: {
                                          pattern:
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                        },
                                        message:
                                          'must match pattern "' +
                                          "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                          '"',
                                      },
                                    ];
                                    return false;
                                  }
                                }
                              }
                            } else {
                              validate64.errors = [
                                {
                                  instancePath: instancePath + "/generation",
                                  schemaPath: "#/$defs/Id/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs21 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.nativeRunId !== undefined) {
                            let data8 = data.nativeRunId;
                            const _errs24 = errors;
                            const _errs25 = errors;
                            if (errors === _errs25) {
                              if (typeof data8 === "string") {
                                if (func1(data8) > 128) {
                                  validate64.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/nativeRunId",
                                      schemaPath: "#/$defs/Id/maxLength",
                                      keyword: "maxLength",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (func1(data8) < 1) {
                                    validate64.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/nativeRunId",
                                        schemaPath: "#/$defs/Id/minLength",
                                        keyword: "minLength",
                                        params: { limit: 1 },
                                        message:
                                          "must NOT have fewer than 1 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (!pattern4.test(data8)) {
                                      validate64.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/nativeRunId",
                                          schemaPath: "#/$defs/Id/pattern",
                                          keyword: "pattern",
                                          params: {
                                            pattern:
                                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                          },
                                          message:
                                            'must match pattern "' +
                                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                            '"',
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                }
                              } else {
                                validate64.errors = [
                                  {
                                    instancePath: instancePath + "/nativeRunId",
                                    schemaPath: "#/$defs/Id/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs24 === errors;
                          } else {
                            var valid0 = true;
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate64.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate64.errors = vErrors;
  return errors === 0;
}
validate64.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema146 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "snapshotPage",
      type: "string",
      description: "Closed record discriminator.",
    },
    snapshotId: {
      $ref: "#/$defs/Id",
      description: "Identity shared by all pages from one immutable read view.",
    },
    pageIndex: {
      $ref: "#/$defs/Counter",
      description: "Zero-based page order within this snapshot.",
    },
    session: {
      $ref: "#/$defs/Session",
      description: "Session at the snapshot watermark.",
    },
    cursor: {
      $ref: "#/$defs/Counter",
      description: "Stable event watermark shared by every page.",
    },
    events: {
      type: "array",
      items: { $ref: "#/$defs/Event" },
      description: "Stable events at or below the watermark.",
    },
    commands: {
      type: "array",
      items: { $ref: "#/$defs/CommandRecord" },
      description: "Command projections at the watermark.",
    },
    interactions: {
      type: "array",
      items: { $ref: "#/$defs/Interaction" },
      description:
        "Interaction display state; does not restore a native callback.",
    },
    surfaces: {
      type: "array",
      items: { $ref: "#/$defs/SurfaceState" },
      description:
        "Bounded original A2UI recovery messages and their associations.",
    },
    next: {
      $ref: "#/$defs/Id",
      description: "Opaque continuation; absent at end of the read view.",
    },
  },
  required: [
    "schemaVersion",
    "kind",
    "snapshotId",
    "pageIndex",
    "session",
    "cursor",
    "events",
    "commands",
    "interactions",
    "surfaces",
  ],
  additionalProperties: false,
};
function validate66(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate66.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.snapshotId === undefined && (missing0 = "snapshotId")) ||
        (data.pageIndex === undefined && (missing0 = "pageIndex")) ||
        (data.session === undefined && (missing0 = "session")) ||
        (data.cursor === undefined && (missing0 = "cursor")) ||
        (data.events === undefined && (missing0 = "events")) ||
        (data.commands === undefined && (missing0 = "commands")) ||
        (data.interactions === undefined && (missing0 = "interactions")) ||
        (data.surfaces === undefined && (missing0 = "surfaces"))
      ) {
        validate66.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!func22.call(schema146.properties, key0)) {
            validate66.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate66.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate66.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate66.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("snapshotPage" !== data1) {
                validate66.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "snapshotPage" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.snapshotId !== undefined) {
                let data2 = data.snapshotId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate66.errors = [
                        {
                          instancePath: instancePath + "/snapshotId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate66.errors = [
                          {
                            instancePath: instancePath + "/snapshotId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate66.errors = [
                            {
                              instancePath: instancePath + "/snapshotId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate66.errors = [
                      {
                        instancePath: instancePath + "/snapshotId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.pageIndex !== undefined) {
                  let data3 = data.pageIndex;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (
                    !(
                      typeof data3 == "number" &&
                      !(data3 % 1) &&
                      !isNaN(data3) &&
                      isFinite(data3)
                    )
                  ) {
                    validate66.errors = [
                      {
                        instancePath: instancePath + "/pageIndex",
                        schemaPath: "#/$defs/Counter/type",
                        keyword: "type",
                        params: { type: "integer" },
                        message: "must be integer",
                      },
                    ];
                    return false;
                  }
                  if (errors === _errs10) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 9007199254740991 || isNaN(data3)) {
                        validate66.errors = [
                          {
                            instancePath: instancePath + "/pageIndex",
                            schemaPath: "#/$defs/Counter/maximum",
                            keyword: "maximum",
                            params: {
                              comparison: "<=",
                              limit: 9007199254740991,
                            },
                            message: "must be <= 9007199254740991",
                          },
                        ];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate66.errors = [
                            {
                              instancePath: instancePath + "/pageIndex",
                              schemaPath: "#/$defs/Counter/minimum",
                              keyword: "minimum",
                              params: { comparison: ">=", limit: 0 },
                              message: "must be >= 0",
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.session !== undefined) {
                    const _errs12 = errors;
                    if (
                      !validate48(data.session, {
                        instancePath: instancePath + "/session",
                        parentData: data,
                        parentDataProperty: "session",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate48.errors
                          : vErrors.concat(validate48.errors);
                      errors = vErrors.length;
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.cursor !== undefined) {
                      let data5 = data.cursor;
                      const _errs13 = errors;
                      const _errs14 = errors;
                      if (
                        !(
                          typeof data5 == "number" &&
                          !(data5 % 1) &&
                          !isNaN(data5) &&
                          isFinite(data5)
                        )
                      ) {
                        validate66.errors = [
                          {
                            instancePath: instancePath + "/cursor",
                            schemaPath: "#/$defs/Counter/type",
                            keyword: "type",
                            params: { type: "integer" },
                            message: "must be integer",
                          },
                        ];
                        return false;
                      }
                      if (errors === _errs14) {
                        if (typeof data5 == "number" && isFinite(data5)) {
                          if (data5 > 9007199254740991 || isNaN(data5)) {
                            validate66.errors = [
                              {
                                instancePath: instancePath + "/cursor",
                                schemaPath: "#/$defs/Counter/maximum",
                                keyword: "maximum",
                                params: {
                                  comparison: "<=",
                                  limit: 9007199254740991,
                                },
                                message: "must be <= 9007199254740991",
                              },
                            ];
                            return false;
                          } else {
                            if (data5 < 0 || isNaN(data5)) {
                              validate66.errors = [
                                {
                                  instancePath: instancePath + "/cursor",
                                  schemaPath: "#/$defs/Counter/minimum",
                                  keyword: "minimum",
                                  params: { comparison: ">=", limit: 0 },
                                  message: "must be >= 0",
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      }
                      var valid0 = _errs13 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.events !== undefined) {
                        let data6 = data.events;
                        const _errs16 = errors;
                        if (errors === _errs16) {
                          if (Array.isArray(data6)) {
                            var valid4 = true;
                            const len0 = data6.length;
                            for (let i0 = 0; i0 < len0; i0++) {
                              const _errs18 = errors;
                              if (
                                !validate39(data6[i0], {
                                  instancePath: instancePath + "/events/" + i0,
                                  parentData: data6,
                                  parentDataProperty: i0,
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? validate39.errors
                                    : vErrors.concat(validate39.errors);
                                errors = vErrors.length;
                              }
                              var valid4 = _errs18 === errors;
                              if (!valid4) {
                                break;
                              }
                            }
                          } else {
                            validate66.errors = [
                              {
                                instancePath: instancePath + "/events",
                                schemaPath: "#/properties/events/type",
                                keyword: "type",
                                params: { type: "array" },
                                message: "must be array",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs16 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.commands !== undefined) {
                          let data8 = data.commands;
                          const _errs19 = errors;
                          if (errors === _errs19) {
                            if (Array.isArray(data8)) {
                              var valid5 = true;
                              const len1 = data8.length;
                              for (let i1 = 0; i1 < len1; i1++) {
                                const _errs21 = errors;
                                if (
                                  !validate31(data8[i1], {
                                    instancePath:
                                      instancePath + "/commands/" + i1,
                                    parentData: data8,
                                    parentDataProperty: i1,
                                    rootData,
                                    dynamicAnchors,
                                  })
                                ) {
                                  vErrors =
                                    vErrors === null
                                      ? validate31.errors
                                      : vErrors.concat(validate31.errors);
                                  errors = vErrors.length;
                                }
                                var valid5 = _errs21 === errors;
                                if (!valid5) {
                                  break;
                                }
                              }
                            } else {
                              validate66.errors = [
                                {
                                  instancePath: instancePath + "/commands",
                                  schemaPath: "#/properties/commands/type",
                                  keyword: "type",
                                  params: { type: "array" },
                                  message: "must be array",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs19 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.interactions !== undefined) {
                            let data10 = data.interactions;
                            const _errs22 = errors;
                            if (errors === _errs22) {
                              if (Array.isArray(data10)) {
                                var valid6 = true;
                                const len2 = data10.length;
                                for (let i2 = 0; i2 < len2; i2++) {
                                  const _errs24 = errors;
                                  if (
                                    !validate57(data10[i2], {
                                      instancePath:
                                        instancePath + "/interactions/" + i2,
                                      parentData: data10,
                                      parentDataProperty: i2,
                                      rootData,
                                      dynamicAnchors,
                                    })
                                  ) {
                                    vErrors =
                                      vErrors === null
                                        ? validate57.errors
                                        : vErrors.concat(validate57.errors);
                                    errors = vErrors.length;
                                  }
                                  var valid6 = _errs24 === errors;
                                  if (!valid6) {
                                    break;
                                  }
                                }
                              } else {
                                validate66.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/interactions",
                                    schemaPath:
                                      "#/properties/interactions/type",
                                    keyword: "type",
                                    params: { type: "array" },
                                    message: "must be array",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs22 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.surfaces !== undefined) {
                              let data12 = data.surfaces;
                              const _errs25 = errors;
                              if (errors === _errs25) {
                                if (Array.isArray(data12)) {
                                  var valid7 = true;
                                  const len3 = data12.length;
                                  for (let i3 = 0; i3 < len3; i3++) {
                                    const _errs27 = errors;
                                    if (
                                      !validate43(data12[i3], {
                                        instancePath:
                                          instancePath + "/surfaces/" + i3,
                                        parentData: data12,
                                        parentDataProperty: i3,
                                        rootData,
                                        dynamicAnchors,
                                      })
                                    ) {
                                      vErrors =
                                        vErrors === null
                                          ? validate43.errors
                                          : vErrors.concat(validate43.errors);
                                      errors = vErrors.length;
                                    }
                                    var valid7 = _errs27 === errors;
                                    if (!valid7) {
                                      break;
                                    }
                                  }
                                } else {
                                  validate66.errors = [
                                    {
                                      instancePath: instancePath + "/surfaces",
                                      schemaPath: "#/properties/surfaces/type",
                                      keyword: "type",
                                      params: { type: "array" },
                                      message: "must be array",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid0 = _errs25 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.next !== undefined) {
                                let data14 = data.next;
                                const _errs28 = errors;
                                const _errs29 = errors;
                                if (errors === _errs29) {
                                  if (typeof data14 === "string") {
                                    if (func1(data14) > 128) {
                                      validate66.errors = [
                                        {
                                          instancePath: instancePath + "/next",
                                          schemaPath: "#/$defs/Id/maxLength",
                                          keyword: "maxLength",
                                          params: { limit: 128 },
                                          message:
                                            "must NOT have more than 128 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (func1(data14) < 1) {
                                        validate66.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/next",
                                            schemaPath: "#/$defs/Id/minLength",
                                            keyword: "minLength",
                                            params: { limit: 1 },
                                            message:
                                              "must NOT have fewer than 1 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (!pattern4.test(data14)) {
                                          validate66.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/next",
                                              schemaPath: "#/$defs/Id/pattern",
                                              keyword: "pattern",
                                              params: {
                                                pattern:
                                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                              },
                                              message:
                                                'must match pattern "' +
                                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                                '"',
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                    }
                                  } else {
                                    validate66.errors = [
                                      {
                                        instancePath: instancePath + "/next",
                                        schemaPath: "#/$defs/Id/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid0 = _errs28 === errors;
                              } else {
                                var valid0 = true;
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate66.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate66.errors = vErrors;
  return errors === 0;
}
validate66.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema151 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "sessionPage",
      type: "string",
      description: "Closed record discriminator.",
    },
    items: {
      type: "array",
      items: { $ref: "#/$defs/Session" },
      description: "Caller-scoped sessions in this immutable page.",
    },
    next: {
      $ref: "#/$defs/Id",
      description: "Opaque continuation; absent at end of the read view.",
    },
  },
  required: ["schemaVersion", "kind", "items"],
  additionalProperties: false,
};
function validate73(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate73.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.items === undefined && (missing0 = "items"))
      ) {
        validate73.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "items" ||
              key0 === "next"
            )
          ) {
            validate73.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate73.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate73.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate73.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("sessionPage" !== data1) {
                validate73.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "sessionPage" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.items !== undefined) {
                let data2 = data.items;
                const _errs6 = errors;
                if (errors === _errs6) {
                  if (Array.isArray(data2)) {
                    var valid1 = true;
                    const len0 = data2.length;
                    for (let i0 = 0; i0 < len0; i0++) {
                      const _errs8 = errors;
                      if (
                        !validate48(data2[i0], {
                          instancePath: instancePath + "/items/" + i0,
                          parentData: data2,
                          parentDataProperty: i0,
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate48.errors
                            : vErrors.concat(validate48.errors);
                        errors = vErrors.length;
                      }
                      var valid1 = _errs8 === errors;
                      if (!valid1) {
                        break;
                      }
                    }
                  } else {
                    validate73.errors = [
                      {
                        instancePath: instancePath + "/items",
                        schemaPath: "#/properties/items/type",
                        keyword: "type",
                        params: { type: "array" },
                        message: "must be array",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.next !== undefined) {
                  let data4 = data.next;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data4 === "string") {
                      if (func1(data4) > 128) {
                        validate73.errors = [
                          {
                            instancePath: instancePath + "/next",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data4) < 1) {
                          validate73.errors = [
                            {
                              instancePath: instancePath + "/next",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data4)) {
                            validate73.errors = [
                              {
                                instancePath: instancePath + "/next",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate73.errors = [
                        {
                          instancePath: instancePath + "/next",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
              }
            }
          }
        }
      }
    } else {
      validate73.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate73.errors = vErrors;
  return errors === 0;
}
validate73.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema153 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "snapshotRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
    query: {
      $ref: "#/$defs/PageQuery",
      description:
        "Bounded page query with an opaque caller-bound continuation.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId", "query"],
  additionalProperties: false,
};
const schema155 = {
  type: "object",
  properties: {
    limit: {
      type: "integer",
      minimum: 1,
      maximum: 256,
      description: "Maximum records in this page, from 1 to 256.",
    },
    continuation: {
      $ref: "#/$defs/Id",
      description:
        "Opaque continuation of one immutable read view; expires independently of the session.",
    },
  },
  required: ["limit"],
  additionalProperties: false,
};
function validate77(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate77.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.limit === undefined && (missing0 = "limit")) {
        validate77.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "limit" || key0 === "continuation")) {
            validate77.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.limit !== undefined) {
            let data0 = data.limit;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate77.errors = [
                {
                  instancePath: instancePath + "/limit",
                  schemaPath: "#/properties/limit/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (errors === _errs2) {
              if (typeof data0 == "number" && isFinite(data0)) {
                if (data0 > 256 || isNaN(data0)) {
                  validate77.errors = [
                    {
                      instancePath: instancePath + "/limit",
                      schemaPath: "#/properties/limit/maximum",
                      keyword: "maximum",
                      params: { comparison: "<=", limit: 256 },
                      message: "must be <= 256",
                    },
                  ];
                  return false;
                } else {
                  if (data0 < 1 || isNaN(data0)) {
                    validate77.errors = [
                      {
                        instancePath: instancePath + "/limit",
                        schemaPath: "#/properties/limit/minimum",
                        keyword: "minimum",
                        params: { comparison: ">=", limit: 1 },
                        message: "must be >= 1",
                      },
                    ];
                    return false;
                  }
                }
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.continuation !== undefined) {
              let data1 = data.continuation;
              const _errs4 = errors;
              const _errs5 = errors;
              if (errors === _errs5) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate77.errors = [
                      {
                        instancePath: instancePath + "/continuation",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate77.errors = [
                        {
                          instancePath: instancePath + "/continuation",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate77.errors = [
                          {
                            instancePath: instancePath + "/continuation",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate77.errors = [
                    {
                      instancePath: instancePath + "/continuation",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate77.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate77.errors = vErrors;
  return errors === 0;
}
validate77.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate76(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate76.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.query === undefined && (missing0 = "query"))
      ) {
        validate76.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "query"
            )
          ) {
            validate76.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate76.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate76.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate76.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("snapshotRequest" !== data1) {
                validate76.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "snapshotRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate76.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate76.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate76.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate76.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.query !== undefined) {
                  const _errs9 = errors;
                  if (
                    !validate77(data.query, {
                      instancePath: instancePath + "/query",
                      parentData: data,
                      parentDataProperty: "query",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate77.errors
                        : vErrors.concat(validate77.errors);
                    errors = vErrors.length;
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
              }
            }
          }
        }
      }
    } else {
      validate76.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate76.errors = vErrors;
  return errors === 0;
}
validate76.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema157 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "listRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    query: {
      $ref: "#/$defs/PageQuery",
      description:
        "Bounded page query with an opaque caller-bound continuation.",
    },
  },
  required: ["schemaVersion", "kind", "query"],
  additionalProperties: false,
};
function validate80(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate80.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.query === undefined && (missing0 = "query"))
      ) {
        validate80.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(key0 === "schemaVersion" || key0 === "kind" || key0 === "query")
          ) {
            validate80.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate80.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate80.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate80.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("listRequest" !== data1) {
                validate80.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "listRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.query !== undefined) {
                const _errs6 = errors;
                if (
                  !validate77(data.query, {
                    instancePath: instancePath + "/query",
                    parentData: data,
                    parentDataProperty: "query",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate77.errors
                      : vErrors.concat(validate77.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate80.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate80.errors = vErrors;
  return errors === 0;
}
validate80.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema158 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "attachRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
    after: {
      $ref: "#/$defs/Counter",
      description: "Last stable sequence consumed before attaching.",
    },
    attachmentId: {
      $ref: "#/$defs/Id",
      description:
        "Connection-local attachment identity, echoed on every update.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId", "after", "attachmentId"],
  additionalProperties: false,
};
function validate83(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate83.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.after === undefined && (missing0 = "after")) ||
        (data.attachmentId === undefined && (missing0 = "attachmentId"))
      ) {
        validate83.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "after" ||
              key0 === "attachmentId"
            )
          ) {
            validate83.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate83.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate83.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate83.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("attachRequest" !== data1) {
                validate83.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "attachRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate83.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate83.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate83.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate83.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.after !== undefined) {
                  let data3 = data.after;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (
                    !(
                      typeof data3 == "number" &&
                      !(data3 % 1) &&
                      !isNaN(data3) &&
                      isFinite(data3)
                    )
                  ) {
                    validate83.errors = [
                      {
                        instancePath: instancePath + "/after",
                        schemaPath: "#/$defs/Counter/type",
                        keyword: "type",
                        params: { type: "integer" },
                        message: "must be integer",
                      },
                    ];
                    return false;
                  }
                  if (errors === _errs10) {
                    if (typeof data3 == "number" && isFinite(data3)) {
                      if (data3 > 9007199254740991 || isNaN(data3)) {
                        validate83.errors = [
                          {
                            instancePath: instancePath + "/after",
                            schemaPath: "#/$defs/Counter/maximum",
                            keyword: "maximum",
                            params: {
                              comparison: "<=",
                              limit: 9007199254740991,
                            },
                            message: "must be <= 9007199254740991",
                          },
                        ];
                        return false;
                      } else {
                        if (data3 < 0 || isNaN(data3)) {
                          validate83.errors = [
                            {
                              instancePath: instancePath + "/after",
                              schemaPath: "#/$defs/Counter/minimum",
                              keyword: "minimum",
                              params: { comparison: ">=", limit: 0 },
                              message: "must be >= 0",
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.attachmentId !== undefined) {
                    let data4 = data.attachmentId;
                    const _errs12 = errors;
                    const _errs13 = errors;
                    if (errors === _errs13) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate83.errors = [
                            {
                              instancePath: instancePath + "/attachmentId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate83.errors = [
                              {
                                instancePath: instancePath + "/attachmentId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate83.errors = [
                                {
                                  instancePath: instancePath + "/attachmentId",
                                  schemaPath: "#/$defs/Id/pattern",
                                  keyword: "pattern",
                                  params: {
                                    pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                  },
                                  message:
                                    'must match pattern "' +
                                    "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                    '"',
                                },
                              ];
                              return false;
                            }
                          }
                        }
                      } else {
                        validate83.errors = [
                          {
                            instancePath: instancePath + "/attachmentId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate83.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate83.errors = vErrors;
  return errors === 0;
}
validate83.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema162 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "detachRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
    attachmentId: {
      $ref: "#/$defs/Id",
      description:
        "Connection-local attachment identity, echoed on every update.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId", "attachmentId"],
  additionalProperties: false,
};
function validate85(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate85.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.attachmentId === undefined && (missing0 = "attachmentId"))
      ) {
        validate85.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "attachmentId"
            )
          ) {
            validate85.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate85.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate85.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate85.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("detachRequest" !== data1) {
                validate85.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "detachRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate85.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate85.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate85.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate85.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.attachmentId !== undefined) {
                  let data3 = data.attachmentId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate85.errors = [
                          {
                            instancePath: instancePath + "/attachmentId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate85.errors = [
                            {
                              instancePath: instancePath + "/attachmentId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate85.errors = [
                              {
                                instancePath: instancePath + "/attachmentId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate85.errors = [
                        {
                          instancePath: instancePath + "/attachmentId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
              }
            }
          }
        }
      }
    } else {
      validate85.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate85.errors = vErrors;
  return errors === 0;
}
validate85.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema165 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "resumeRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId"],
  additionalProperties: false,
};
function validate87(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate87.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId"))
      ) {
        validate87.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId"
            )
          ) {
            validate87.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate87.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate87.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate87.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("resumeRequest" !== data1) {
                validate87.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "resumeRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate87.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate87.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate87.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate87.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
            }
          }
        }
      }
    } else {
      validate87.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate87.errors = vErrors;
  return errors === 0;
}
validate87.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema167 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "actionRequest",
      type: "string",
      description: "Closed record discriminator.",
    },
    metadata: {
      $ref: "#/$defs/SurfaceAction",
      description:
        "Product action association, checked independently of untrusted upstream context.",
    },
    message: {
      type: "object",
      additionalProperties: true,
      description:
        "Unchanged upstream A2UI client message, validated against the negotiated schema.",
    },
    expiresAtMs: {
      $ref: "#/$defs/Counter",
      description: "Command expiry in Unix milliseconds.",
    },
  },
  required: ["schemaVersion", "kind", "metadata", "message", "expiresAtMs"],
  additionalProperties: false,
};
function validate89(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate89.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.metadata === undefined && (missing0 = "metadata")) ||
        (data.message === undefined && (missing0 = "message")) ||
        (data.expiresAtMs === undefined && (missing0 = "expiresAtMs"))
      ) {
        validate89.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "metadata" ||
              key0 === "message" ||
              key0 === "expiresAtMs"
            )
          ) {
            validate89.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate89.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate89.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate89.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("actionRequest" !== data1) {
                validate89.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "actionRequest" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.metadata !== undefined) {
                const _errs6 = errors;
                if (
                  !validate64(data.metadata, {
                    instancePath: instancePath + "/metadata",
                    parentData: data,
                    parentDataProperty: "metadata",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate64.errors
                      : vErrors.concat(validate64.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.message !== undefined) {
                  let data3 = data.message;
                  const _errs7 = errors;
                  if (errors === _errs7) {
                    if (
                      data3 &&
                      typeof data3 == "object" &&
                      !Array.isArray(data3)
                    ) {
                    } else {
                      validate89.errors = [
                        {
                          instancePath: instancePath + "/message",
                          schemaPath: "#/properties/message/type",
                          keyword: "type",
                          params: { type: "object" },
                          message: "must be object",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs7 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.expiresAtMs !== undefined) {
                    let data4 = data.expiresAtMs;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (
                      !(
                        typeof data4 == "number" &&
                        !(data4 % 1) &&
                        !isNaN(data4) &&
                        isFinite(data4)
                      )
                    ) {
                      validate89.errors = [
                        {
                          instancePath: instancePath + "/expiresAtMs",
                          schemaPath: "#/$defs/Counter/type",
                          keyword: "type",
                          params: { type: "integer" },
                          message: "must be integer",
                        },
                      ];
                      return false;
                    }
                    if (errors === _errs11) {
                      if (typeof data4 == "number" && isFinite(data4)) {
                        if (data4 > 9007199254740991 || isNaN(data4)) {
                          validate89.errors = [
                            {
                              instancePath: instancePath + "/expiresAtMs",
                              schemaPath: "#/$defs/Counter/maximum",
                              keyword: "maximum",
                              params: {
                                comparison: "<=",
                                limit: 9007199254740991,
                              },
                              message: "must be <= 9007199254740991",
                            },
                          ];
                          return false;
                        } else {
                          if (data4 < 0 || isNaN(data4)) {
                            validate89.errors = [
                              {
                                instancePath: instancePath + "/expiresAtMs",
                                schemaPath: "#/$defs/Counter/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs10 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate89.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate89.errors = vErrors;
  return errors === 0;
}
validate89.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema169 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "accessUpdate",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
    attachmentId: {
      $ref: "#/$defs/Id",
      description:
        "Connection-local attachment identity, echoed on every update.",
    },
    update: {
      $ref: "#/$defs/Subscription",
      description:
        "Stable event, ephemeral delta, or explicit resynchronization signal.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId", "attachmentId", "update"],
  additionalProperties: false,
};
const schema172 = {
  oneOf: [
    {
      type: "object",
      properties: {
        type: {
          const: "event",
          type: "string",
          description: "Closed variant discriminator.",
        },
        event: { $ref: "#/$defs/Event", description: "Stable Host event." },
      },
      required: ["type", "event"],
      additionalProperties: false,
    },
    {
      type: "object",
      properties: {
        type: {
          const: "delta",
          type: "string",
          description: "Closed variant discriminator.",
        },
        generation: {
          $ref: "#/$defs/Id",
          description: "Exact native provider incarnation.",
        },
        commandId: {
          $ref: "#/$defs/Id",
          description: "Product command identity.",
        },
        messageId: {
          $ref: "#/$defs/Id",
          description: "Identity of the streamed message within a command.",
        },
        text: { type: "string", description: "Untrusted display text." },
      },
      required: ["type", "generation", "commandId", "messageId", "text"],
      additionalProperties: false,
    },
    {
      type: "object",
      properties: {
        type: {
          const: "resync_required",
          type: "string",
          description: "Closed variant discriminator.",
        },
      },
      required: ["type"],
      additionalProperties: false,
    },
  ],
};
function validate93(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate93.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.type === undefined && (missing0 = "type")) ||
        (data.event === undefined && (missing0 = "event"))
      ) {
        const err0 = {
          instancePath,
          schemaPath: "#/oneOf/0/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "type" || key0 === "event")) {
            const err1 = {
              instancePath,
              schemaPath: "#/oneOf/0/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.type !== undefined) {
            let data0 = data.type;
            const _errs4 = errors;
            if (typeof data0 !== "string") {
              const err2 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("event" !== data0) {
              const err3 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/0/properties/type/const",
                keyword: "const",
                params: { allowedValue: "event" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.event !== undefined) {
              const _errs6 = errors;
              if (
                !validate39(data.event, {
                  instancePath: instancePath + "/event",
                  parentData: data,
                  parentDataProperty: "event",
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate39.errors
                    : vErrors.concat(validate39.errors);
                errors = vErrors.length;
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err4 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs7 = errors;
  if (errors === _errs7) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (
        (data.type === undefined && (missing1 = "type")) ||
        (data.generation === undefined && (missing1 = "generation")) ||
        (data.commandId === undefined && (missing1 = "commandId")) ||
        (data.messageId === undefined && (missing1 = "messageId")) ||
        (data.text === undefined && (missing1 = "text"))
      ) {
        const err5 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing1 },
          message: "must have required property '" + missing1 + "'",
        };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      } else {
        const _errs9 = errors;
        for (const key1 in data) {
          if (
            !(
              key1 === "type" ||
              key1 === "generation" ||
              key1 === "commandId" ||
              key1 === "messageId" ||
              key1 === "text"
            )
          ) {
            const err6 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key1 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err6];
            } else {
              vErrors.push(err6);
            }
            errors++;
            break;
          }
        }
        if (_errs9 === errors) {
          if (data.type !== undefined) {
            let data2 = data.type;
            const _errs10 = errors;
            if (typeof data2 !== "string") {
              const err7 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err7];
              } else {
                vErrors.push(err7);
              }
              errors++;
            }
            if ("delta" !== data2) {
              const err8 = {
                instancePath: instancePath + "/type",
                schemaPath: "#/oneOf/1/properties/type/const",
                keyword: "const",
                params: { allowedValue: "delta" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
            }
            var valid2 = _errs10 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.generation !== undefined) {
              let data3 = data.generation;
              const _errs12 = errors;
              const _errs13 = errors;
              if (errors === _errs13) {
                if (typeof data3 === "string") {
                  if (func1(data3) > 128) {
                    const err9 = {
                      instancePath: instancePath + "/generation",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err9];
                    } else {
                      vErrors.push(err9);
                    }
                    errors++;
                  } else {
                    if (func1(data3) < 1) {
                      const err10 = {
                        instancePath: instancePath + "/generation",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err10];
                      } else {
                        vErrors.push(err10);
                      }
                      errors++;
                    } else {
                      if (!pattern4.test(data3)) {
                        const err11 = {
                          instancePath: instancePath + "/generation",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: {
                            pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                          },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                            '"',
                        };
                        if (vErrors === null) {
                          vErrors = [err11];
                        } else {
                          vErrors.push(err11);
                        }
                        errors++;
                      }
                    }
                  }
                } else {
                  const err12 = {
                    instancePath: instancePath + "/generation",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err12];
                  } else {
                    vErrors.push(err12);
                  }
                  errors++;
                }
              }
              var valid2 = _errs12 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.commandId !== undefined) {
                let data4 = data.commandId;
                const _errs15 = errors;
                const _errs16 = errors;
                if (errors === _errs16) {
                  if (typeof data4 === "string") {
                    if (func1(data4) > 128) {
                      const err13 = {
                        instancePath: instancePath + "/commandId",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err13];
                      } else {
                        vErrors.push(err13);
                      }
                      errors++;
                    } else {
                      if (func1(data4) < 1) {
                        const err14 = {
                          instancePath: instancePath + "/commandId",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err14];
                        } else {
                          vErrors.push(err14);
                        }
                        errors++;
                      } else {
                        if (!pattern4.test(data4)) {
                          const err15 = {
                            instancePath: instancePath + "/commandId",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                              '"',
                          };
                          if (vErrors === null) {
                            vErrors = [err15];
                          } else {
                            vErrors.push(err15);
                          }
                          errors++;
                        }
                      }
                    }
                  } else {
                    const err16 = {
                      instancePath: instancePath + "/commandId",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err16];
                    } else {
                      vErrors.push(err16);
                    }
                    errors++;
                  }
                }
                var valid2 = _errs15 === errors;
              } else {
                var valid2 = true;
              }
              if (valid2) {
                if (data.messageId !== undefined) {
                  let data5 = data.messageId;
                  const _errs18 = errors;
                  const _errs19 = errors;
                  if (errors === _errs19) {
                    if (typeof data5 === "string") {
                      if (func1(data5) > 128) {
                        const err17 = {
                          instancePath: instancePath + "/messageId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err17];
                        } else {
                          vErrors.push(err17);
                        }
                        errors++;
                      } else {
                        if (func1(data5) < 1) {
                          const err18 = {
                            instancePath: instancePath + "/messageId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          };
                          if (vErrors === null) {
                            vErrors = [err18];
                          } else {
                            vErrors.push(err18);
                          }
                          errors++;
                        } else {
                          if (!pattern4.test(data5)) {
                            const err19 = {
                              instancePath: instancePath + "/messageId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            };
                            if (vErrors === null) {
                              vErrors = [err19];
                            } else {
                              vErrors.push(err19);
                            }
                            errors++;
                          }
                        }
                      }
                    } else {
                      const err20 = {
                        instancePath: instancePath + "/messageId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err20];
                      } else {
                        vErrors.push(err20);
                      }
                      errors++;
                    }
                  }
                  var valid2 = _errs18 === errors;
                } else {
                  var valid2 = true;
                }
                if (valid2) {
                  if (data.text !== undefined) {
                    const _errs21 = errors;
                    if (typeof data.text !== "string") {
                      const err21 = {
                        instancePath: instancePath + "/text",
                        schemaPath: "#/oneOf/1/properties/text/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      };
                      if (vErrors === null) {
                        vErrors = [err21];
                      } else {
                        vErrors.push(err21);
                      }
                      errors++;
                    }
                    var valid2 = _errs21 === errors;
                  } else {
                    var valid2 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      const err22 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err22];
      } else {
        vErrors.push(err22);
      }
      errors++;
    }
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs23 = errors;
    if (errors === _errs23) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing2;
        if (data.type === undefined && (missing2 = "type")) {
          const err23 = {
            instancePath,
            schemaPath: "#/oneOf/2/required",
            keyword: "required",
            params: { missingProperty: missing2 },
            message: "must have required property '" + missing2 + "'",
          };
          if (vErrors === null) {
            vErrors = [err23];
          } else {
            vErrors.push(err23);
          }
          errors++;
        } else {
          const _errs25 = errors;
          for (const key2 in data) {
            if (!(key2 === "type")) {
              const err24 = {
                instancePath,
                schemaPath: "#/oneOf/2/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key2 },
                message: "must NOT have additional properties",
              };
              if (vErrors === null) {
                vErrors = [err24];
              } else {
                vErrors.push(err24);
              }
              errors++;
              break;
            }
          }
          if (_errs25 === errors) {
            if (data.type !== undefined) {
              let data7 = data.type;
              if (typeof data7 !== "string") {
                const err25 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err25];
                } else {
                  vErrors.push(err25);
                }
                errors++;
              }
              if ("resync_required" !== data7) {
                const err26 = {
                  instancePath: instancePath + "/type",
                  schemaPath: "#/oneOf/2/properties/type/const",
                  keyword: "const",
                  params: { allowedValue: "resync_required" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err26];
                } else {
                  vErrors.push(err26);
                }
                errors++;
              }
            }
          }
        }
      } else {
        const err27 = {
          instancePath,
          schemaPath: "#/oneOf/2/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        };
        if (vErrors === null) {
          vErrors = [err27];
        } else {
          vErrors.push(err27);
        }
        errors++;
      }
    }
    var _valid0 = _errs23 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
    }
  }
  if (!valid0) {
    const err28 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err28];
    } else {
      vErrors.push(err28);
    }
    errors++;
    validate93.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate93.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate93.evaluated = { dynamicProps: true, dynamicItems: false };
function validate92(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate92.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.attachmentId === undefined && (missing0 = "attachmentId")) ||
        (data.update === undefined && (missing0 = "update"))
      ) {
        validate92.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "attachmentId" ||
              key0 === "update"
            )
          ) {
            validate92.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate92.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate92.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate92.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("accessUpdate" !== data1) {
                validate92.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "accessUpdate" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate92.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate92.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate92.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate92.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.attachmentId !== undefined) {
                  let data3 = data.attachmentId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate92.errors = [
                          {
                            instancePath: instancePath + "/attachmentId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate92.errors = [
                            {
                              instancePath: instancePath + "/attachmentId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate92.errors = [
                              {
                                instancePath: instancePath + "/attachmentId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate92.errors = [
                        {
                          instancePath: instancePath + "/attachmentId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.update !== undefined) {
                    const _errs12 = errors;
                    if (
                      !validate93(data.update, {
                        instancePath: instancePath + "/update",
                        parentData: data,
                        parentDataProperty: "update",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate93.errors
                          : vErrors.concat(validate93.errors);
                      errors = vErrors.length;
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate92.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate92.errors = vErrors;
  return errors === 0;
}
validate92.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema176 = {
  type: "object",
  properties: {
    schemaVersion: {
      const: 2,
      type: "integer",
      description: "Exact product contract version; no legacy readers.",
    },
    kind: {
      const: "attachReceipt",
      type: "string",
      description: "Closed record discriminator.",
    },
    sessionId: {
      $ref: "#/$defs/Id",
      description:
        "Product session identity within the authenticated caller namespace.",
    },
    attachmentId: {
      $ref: "#/$defs/Id",
      description:
        "Connection-local attachment identity, echoed on every update.",
    },
    after: {
      $ref: "#/$defs/Counter",
      description: "Last stable sequence consumed before attaching.",
    },
  },
  required: ["schemaVersion", "kind", "sessionId", "attachmentId", "after"],
  additionalProperties: false,
};
function validate97(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate97.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.schemaVersion === undefined && (missing0 = "schemaVersion")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.sessionId === undefined && (missing0 = "sessionId")) ||
        (data.attachmentId === undefined && (missing0 = "attachmentId")) ||
        (data.after === undefined && (missing0 = "after"))
      ) {
        validate97.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (
            !(
              key0 === "schemaVersion" ||
              key0 === "kind" ||
              key0 === "sessionId" ||
              key0 === "attachmentId" ||
              key0 === "after"
            )
          ) {
            validate97.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.schemaVersion !== undefined) {
            let data0 = data.schemaVersion;
            const _errs2 = errors;
            if (
              !(
                typeof data0 == "number" &&
                !(data0 % 1) &&
                !isNaN(data0) &&
                isFinite(data0)
              )
            ) {
              validate97.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/type",
                  keyword: "type",
                  params: { type: "integer" },
                  message: "must be integer",
                },
              ];
              return false;
            }
            if (2 !== data0) {
              validate97.errors = [
                {
                  instancePath: instancePath + "/schemaVersion",
                  schemaPath: "#/properties/schemaVersion/const",
                  keyword: "const",
                  params: { allowedValue: 2 },
                  message: "must be equal to constant",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs4 = errors;
              if (typeof data1 !== "string") {
                validate97.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if ("attachReceipt" !== data1) {
                validate97.errors = [
                  {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "attachReceipt" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.sessionId !== undefined) {
                let data2 = data.sessionId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate97.errors = [
                        {
                          instancePath: instancePath + "/sessionId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate97.errors = [
                          {
                            instancePath: instancePath + "/sessionId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate97.errors = [
                            {
                              instancePath: instancePath + "/sessionId",
                              schemaPath: "#/$defs/Id/pattern",
                              keyword: "pattern",
                              params: {
                                pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                              },
                              message:
                                'must match pattern "' +
                                "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate97.errors = [
                      {
                        instancePath: instancePath + "/sessionId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.attachmentId !== undefined) {
                  let data3 = data.attachmentId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate97.errors = [
                          {
                            instancePath: instancePath + "/attachmentId",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate97.errors = [
                            {
                              instancePath: instancePath + "/attachmentId",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate97.errors = [
                              {
                                instancePath: instancePath + "/attachmentId",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
                                },
                                message:
                                  'must match pattern "' +
                                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                                  '"',
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    } else {
                      validate97.errors = [
                        {
                          instancePath: instancePath + "/attachmentId",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.after !== undefined) {
                    let data4 = data.after;
                    const _errs12 = errors;
                    const _errs13 = errors;
                    if (
                      !(
                        typeof data4 == "number" &&
                        !(data4 % 1) &&
                        !isNaN(data4) &&
                        isFinite(data4)
                      )
                    ) {
                      validate97.errors = [
                        {
                          instancePath: instancePath + "/after",
                          schemaPath: "#/$defs/Counter/type",
                          keyword: "type",
                          params: { type: "integer" },
                          message: "must be integer",
                        },
                      ];
                      return false;
                    }
                    if (errors === _errs13) {
                      if (typeof data4 == "number" && isFinite(data4)) {
                        if (data4 > 9007199254740991 || isNaN(data4)) {
                          validate97.errors = [
                            {
                              instancePath: instancePath + "/after",
                              schemaPath: "#/$defs/Counter/maximum",
                              keyword: "maximum",
                              params: {
                                comparison: "<=",
                                limit: 9007199254740991,
                              },
                              message: "must be <= 9007199254740991",
                            },
                          ];
                          return false;
                        } else {
                          if (data4 < 0 || isNaN(data4)) {
                            validate97.errors = [
                              {
                                instancePath: instancePath + "/after",
                                schemaPath: "#/$defs/Counter/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate97.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate97.errors = vErrors;
  return errors === 0;
}
validate97.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate20(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  /*# sourceURL="urn:rss-mdm-agent:ai-runtime:2" */ let vErrors = null;
  let errors = 0;
  const evaluated0 = validate20.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (
    !validate21(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate21.errors : vErrors.concat(validate21.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs2 = errors;
  if (
    !validate27(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate27.errors : vErrors.concat(validate27.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs2 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs3 = errors;
    if (
      !validate31(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate31.errors
          : vErrors.concat(validate31.errors);
      errors = vErrors.length;
    }
    var _valid0 = _errs3 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs4 = errors;
      if (
        !validate39(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate39.errors
            : vErrors.concat(validate39.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs4 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs5 = errors;
        if (
          !validate48(data, {
            instancePath,
            parentData,
            parentDataProperty,
            rootData,
            dynamicAnchors,
          })
        ) {
          vErrors =
            vErrors === null
              ? validate48.errors
              : vErrors.concat(validate48.errors);
          errors = vErrors.length;
        }
        var _valid0 = _errs5 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs6 = errors;
          if (
            !validate57(data, {
              instancePath,
              parentData,
              parentDataProperty,
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? validate57.errors
                : vErrors.concat(validate57.errors);
            errors = vErrors.length;
          }
          var _valid0 = _errs6 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs7 = errors;
            if (
              !validate60(data, {
                instancePath,
                parentData,
                parentDataProperty,
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate60.errors
                  : vErrors.concat(validate60.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs7 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs8 = errors;
              if (
                !validate43(data, {
                  instancePath,
                  parentData,
                  parentDataProperty,
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate43.errors
                    : vErrors.concat(validate43.errors);
                errors = vErrors.length;
              }
              var _valid0 = _errs8 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs9 = errors;
                if (
                  !validate64(data, {
                    instancePath,
                    parentData,
                    parentDataProperty,
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate64.errors
                      : vErrors.concat(validate64.errors);
                  errors = vErrors.length;
                }
                var _valid0 = _errs9 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs10 = errors;
                  if (
                    !validate66(data, {
                      instancePath,
                      parentData,
                      parentDataProperty,
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate66.errors
                        : vErrors.concat(validate66.errors);
                    errors = vErrors.length;
                  }
                  var _valid0 = _errs10 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs11 = errors;
                    if (
                      !validate73(data, {
                        instancePath,
                        parentData,
                        parentDataProperty,
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate73.errors
                          : vErrors.concat(validate73.errors);
                      errors = vErrors.length;
                    }
                    var _valid0 = _errs11 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                      const _errs12 = errors;
                      if (
                        !validate76(data, {
                          instancePath,
                          parentData,
                          parentDataProperty,
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate76.errors
                            : vErrors.concat(validate76.errors);
                        errors = vErrors.length;
                      }
                      var _valid0 = _errs12 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props0 !== true) {
                            props0 = true;
                          }
                        }
                        const _errs13 = errors;
                        if (
                          !validate80(data, {
                            instancePath,
                            parentData,
                            parentDataProperty,
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate80.errors
                              : vErrors.concat(validate80.errors);
                          errors = vErrors.length;
                        }
                        var _valid0 = _errs13 === errors;
                        if (_valid0 && valid0) {
                          valid0 = false;
                          passing0 = [passing0, 12];
                        } else {
                          if (_valid0) {
                            valid0 = true;
                            passing0 = 12;
                            if (props0 !== true) {
                              props0 = true;
                            }
                          }
                          const _errs14 = errors;
                          if (
                            !validate83(data, {
                              instancePath,
                              parentData,
                              parentDataProperty,
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate83.errors
                                : vErrors.concat(validate83.errors);
                            errors = vErrors.length;
                          }
                          var _valid0 = _errs14 === errors;
                          if (_valid0 && valid0) {
                            valid0 = false;
                            passing0 = [passing0, 13];
                          } else {
                            if (_valid0) {
                              valid0 = true;
                              passing0 = 13;
                              if (props0 !== true) {
                                props0 = true;
                              }
                            }
                            const _errs15 = errors;
                            if (
                              !validate85(data, {
                                instancePath,
                                parentData,
                                parentDataProperty,
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? validate85.errors
                                  : vErrors.concat(validate85.errors);
                              errors = vErrors.length;
                            }
                            var _valid0 = _errs15 === errors;
                            if (_valid0 && valid0) {
                              valid0 = false;
                              passing0 = [passing0, 14];
                            } else {
                              if (_valid0) {
                                valid0 = true;
                                passing0 = 14;
                                if (props0 !== true) {
                                  props0 = true;
                                }
                              }
                              const _errs16 = errors;
                              if (
                                !validate87(data, {
                                  instancePath,
                                  parentData,
                                  parentDataProperty,
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? validate87.errors
                                    : vErrors.concat(validate87.errors);
                                errors = vErrors.length;
                              }
                              var _valid0 = _errs16 === errors;
                              if (_valid0 && valid0) {
                                valid0 = false;
                                passing0 = [passing0, 15];
                              } else {
                                if (_valid0) {
                                  valid0 = true;
                                  passing0 = 15;
                                  if (props0 !== true) {
                                    props0 = true;
                                  }
                                }
                                const _errs17 = errors;
                                if (
                                  !validate89(data, {
                                    instancePath,
                                    parentData,
                                    parentDataProperty,
                                    rootData,
                                    dynamicAnchors,
                                  })
                                ) {
                                  vErrors =
                                    vErrors === null
                                      ? validate89.errors
                                      : vErrors.concat(validate89.errors);
                                  errors = vErrors.length;
                                }
                                var _valid0 = _errs17 === errors;
                                if (_valid0 && valid0) {
                                  valid0 = false;
                                  passing0 = [passing0, 16];
                                } else {
                                  if (_valid0) {
                                    valid0 = true;
                                    passing0 = 16;
                                    if (props0 !== true) {
                                      props0 = true;
                                    }
                                  }
                                  const _errs18 = errors;
                                  if (
                                    !validate92(data, {
                                      instancePath,
                                      parentData,
                                      parentDataProperty,
                                      rootData,
                                      dynamicAnchors,
                                    })
                                  ) {
                                    vErrors =
                                      vErrors === null
                                        ? validate92.errors
                                        : vErrors.concat(validate92.errors);
                                    errors = vErrors.length;
                                  }
                                  var _valid0 = _errs18 === errors;
                                  if (_valid0 && valid0) {
                                    valid0 = false;
                                    passing0 = [passing0, 17];
                                  } else {
                                    if (_valid0) {
                                      valid0 = true;
                                      passing0 = 17;
                                      if (props0 !== true) {
                                        props0 = true;
                                      }
                                    }
                                    const _errs19 = errors;
                                    if (
                                      !validate97(data, {
                                        instancePath,
                                        parentData,
                                        parentDataProperty,
                                        rootData,
                                        dynamicAnchors,
                                      })
                                    ) {
                                      vErrors =
                                        vErrors === null
                                          ? validate97.errors
                                          : vErrors.concat(validate97.errors);
                                      errors = vErrors.length;
                                    }
                                    var _valid0 = _errs19 === errors;
                                    if (_valid0 && valid0) {
                                      valid0 = false;
                                      passing0 = [passing0, 18];
                                    } else {
                                      if (_valid0) {
                                        valid0 = true;
                                        passing0 = 18;
                                        if (props0 !== true) {
                                          props0 = true;
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
    validate20.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate20.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate20.evaluated = { dynamicProps: true, dynamicItems: false };
