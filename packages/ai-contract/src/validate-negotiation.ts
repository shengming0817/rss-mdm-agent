// @ts-nocheck
// @generated from canonical schemas; do not edit.

"use strict";
export const validate = validate20;
export default validate20;
const schema31 = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
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
        workspaceId: {
          $ref: "#/$defs/Id",
          description:
            "SHA-256 identity of the normalized absolute workspace path. Filesystem containment remains owned by the provider adapter and composition root.",
        },
        nativeThreadId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads.",
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
        "workspaceId",
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
        "storage_corrupt",
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
    CommandRecord: {
      oneOf: [
        {
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
              type: "string",
              const: "accepted",
              description: "Closed command lifecycle projection.",
            },
          },
          required: ["schemaVersion", "kind", "command", "receipt", "state"],
          additionalProperties: false,
        },
        {
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
              type: "string",
              const: "dispatching",
              description: "Closed command lifecycle projection.",
            },
            dispatch: {
              $ref: "#/$defs/DispatchAttempt",
              description:
                "Original attempt and append-once native correlation coordinates.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "command",
            "receipt",
            "state",
            "dispatch",
          ],
          additionalProperties: false,
        },
        {
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
              type: "string",
              const: "running",
              description: "Closed command lifecycle projection.",
            },
            dispatch: {
              $ref: "#/$defs/DispatchAttempt",
              description:
                "Original attempt and append-once native correlation coordinates.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "command",
            "receipt",
            "state",
            "dispatch",
          ],
          additionalProperties: false,
        },
        {
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
              type: "string",
              const: "terminal",
              description: "Closed command lifecycle projection.",
            },
            dispatch: {
              $ref: "#/$defs/DispatchAttempt",
              description:
                "Original attempt and append-once native correlation coordinates.",
            },
            outcome: {
              $ref: "#/$defs/Outcome",
              description: "Explicitly observed model terminal outcome.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "command",
            "receipt",
            "state",
            "dispatch",
            "outcome",
          ],
          additionalProperties: false,
        },
        {
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
              type: "string",
              const: "reconciliation_required",
              description: "Closed command lifecycle projection.",
            },
            dispatch: {
              $ref: "#/$defs/DispatchAttempt",
              description:
                "Original attempt and append-once native correlation coordinates.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "command",
            "receipt",
            "state",
            "dispatch",
          ],
          additionalProperties: false,
        },
        {
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
              type: "string",
              const: "invalidated",
              description: "Closed command lifecycle projection.",
            },
            failure: {
              $ref: "#/$defs/Failure",
              description: "Local failure without asserting a model terminal.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "command",
            "receipt",
            "state",
            "failure",
          ],
          additionalProperties: false,
        },
      ],
      description:
        "Closed command lifecycle; acceptance is immutable, local invalidation does not assert a model terminal.",
    },
    Event: {
      oneOf: [
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "status",
                  description: "Closed variant discriminator.",
                },
                state: {
                  type: "string",
                  enum: ["accepted"],
                  description: "Closed command lifecycle projection.",
                },
              },
              required: ["type", "state"],
              additionalProperties: false,
              description:
                "status variant; all fields are data, never authentication or execution authority.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "status",
                  description: "Closed variant discriminator.",
                },
                state: {
                  type: "string",
                  enum: ["dispatching", "running", "reconciliation_required"],
                  description: "Closed command lifecycle projection.",
                },
              },
              required: ["type", "state"],
              additionalProperties: false,
              description:
                "status variant; all fields are data, never authentication or execution authority.",
            },
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "tool_proposal",
                  description: "Closed variant discriminator.",
                },
                proposalId: {
                  $ref: "#/$defs/Id",
                  description:
                    "Untrusted tool proposal correlation identifier.",
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "tool_result",
                  description: "Closed variant discriminator.",
                },
                proposalId: {
                  $ref: "#/$defs/Id",
                  description:
                    "Untrusted tool proposal correlation identifier.",
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
                  description:
                    "First publication of an ordinary user question.",
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
                    "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable.",
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
              required: [
                "type",
                "interactionId",
                "status",
                "responseCommandId",
              ],
              additionalProperties: false,
              description:
                "First accepted response identity, committed atomically with the receipt and Interaction.",
            },
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "invalidated",
                  description: "Closed event discriminator.",
                },
                failure: {
                  $ref: "#/$defs/Failure",
                  description:
                    "Local failure without asserting a model terminal.",
                },
              },
              required: ["type", "failure"],
              additionalProperties: false,
              description:
                "Stable event data; never execution or authentication authority.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "dispatch",
                  description: "Closed event discriminator.",
                },
                attempt: {
                  $ref: "#/$defs/DispatchAttempt",
                  description:
                    "Complete dispatch identity retained for replay and reconciliation.",
                },
              },
              required: ["type", "attempt"],
              additionalProperties: false,
              description:
                "Stable event data; never execution or authentication authority.",
            },
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "reconciled",
                  description: "Closed event discriminator.",
                },
                attempt: {
                  $ref: "#/$defs/DispatchAttempt",
                  description:
                    "Complete dispatch identity retained for replay and reconciliation.",
                },
                resolution: {
                  type: "string",
                  enum: ["running", "terminal", "not_submitted", "unknown"],
                  description:
                    "Provider observation bound to this attempt and its current observer.",
                },
              },
              required: ["type", "attempt", "resolution"],
              additionalProperties: false,
              description:
                "Stable event data; never execution or authentication authority.",
            },
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            commandId: {
              $ref: "#/$defs/Id",
              description:
                "Original command identity within the trusted namespace.",
            },
            body: {
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
            attemptId: {
              $ref: "#/$defs/Id",
              description:
                "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "commandId",
            "body",
            "attemptId",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "session_rebound",
                  description: "Closed event discriminator.",
                },
                previousGeneration: {
                  $ref: "#/$defs/Id",
                  description:
                    "Prior provider incarnation invalidated by this verified handoff.",
                },
              },
              required: ["type", "previousGeneration"],
              additionalProperties: false,
              description:
                "Stable event data; never execution or authentication authority.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "body",
          ],
          additionalProperties: false,
        },
        {
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
              description:
                "Stable unique event identifier within the namespace.",
            },
            sequence: {
              $ref: "#/$defs/Counter",
              description:
                "Strictly increasing stable-event counter; attach cursors are exclusive.",
            },
            generation: {
              $ref: "#/$defs/Id",
              description:
                "Live provider incarnation token; rejects callbacks from previous incarnations.",
            },
            body: {
              type: "object",
              properties: {
                type: {
                  type: "string",
                  const: "session_retired",
                  description: "Closed event discriminator.",
                },
              },
              required: ["type"],
              additionalProperties: false,
              description:
                "Stable event data; never execution or authentication authority.",
            },
          },
          required: [
            "schemaVersion",
            "kind",
            "namespace",
            "eventId",
            "sequence",
            "generation",
            "body",
          ],
          additionalProperties: false,
        },
      ],
      description:
        "Closed stable events. Session events have no command, attempt observations name their exact attempt.",
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
            "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable.",
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
    DispatchAttempt: {
      type: "object",
      properties: {
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
          enum: ["intent", "submitted", "unknown"],
          description:
            "intent is stored before native submission; submitted has native acceptance; unknown requires reconciliation.",
        },
        attemptId: {
          $ref: "#/$defs/Id",
          description:
            "Stable identity of one dispatch attempt; never reused after positive non-submission proof.",
        },
        originGeneration: {
          $ref: "#/$defs/Id",
          description:
            "Immutable provider incarnation that originated this attempt.",
        },
        observerGeneration: {
          $ref: "#/$defs/Id",
          description:
            "Current verified provider incarnation permitted to observe this attempt.",
        },
        correlationId: {
          $ref: "#/$defs/Id",
          description:
            "Append-once native lookup key returned for ambiguous submission.",
        },
        nativeThreadId: {
          $ref: "#/$defs/Id",
          description:
            "Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads.",
        },
      },
      required: [
        "attemptId",
        "originGeneration",
        "observerGeneration",
        "nativeSessionId",
        "certainty",
      ],
      additionalProperties: false,
      description:
        "One active dispatch attempt. Origin identity is immutable; unknown native coordinates may be filled once. Only verified rebind changes observerGeneration.",
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
          enum: ["active", "deleted", "invalidated"],
          description:
            "Deleted is an upstream deletion; invalidated is a product-side loss of action authority. Neither may reactivate.",
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
      enum: ["generation_bound"],
      description:
        "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable.",
    },
  },
  $ref: "#/$defs/Negotiation",
};
const schema32 = {
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
      description: "Explicitly selected upstream version and product catalog.",
    },
  },
  required: ["contractVersion", "acp", "durableReceipts", "cursorAttach"],
};
const schema33 = {
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
};
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
        (data.contractVersion === undefined &&
          (missing0 = "contractVersion")) ||
        (data.acp === undefined && (missing0 = "acp")) ||
        (data.durableReceipts === undefined &&
          (missing0 = "durableReceipts")) ||
        (data.cursorAttach === undefined && (missing0 = "cursorAttach"))
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
              key0 === "contractVersion" ||
              key0 === "acp" ||
              key0 === "durableReceipts" ||
              key0 === "cursorAttach" ||
              key0 === "a2ui"
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
          if (data.contractVersion !== undefined) {
            let data0 = data.contractVersion;
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
                  instancePath: instancePath + "/contractVersion",
                  schemaPath: "#/properties/contractVersion/type",
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
                  instancePath: instancePath + "/contractVersion",
                  schemaPath: "#/properties/contractVersion/const",
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
            if (data.acp !== undefined) {
              let data1 = data.acp;
              const _errs4 = errors;
              if (
                !(
                  typeof data1 == "number" &&
                  !(data1 % 1) &&
                  !isNaN(data1) &&
                  isFinite(data1)
                )
              ) {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/acp",
                    schemaPath: "#/properties/acp/type",
                    keyword: "type",
                    params: { type: "integer" },
                    message: "must be integer",
                  },
                ];
                return false;
              }
              if (1 !== data1) {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/acp",
                    schemaPath: "#/properties/acp/const",
                    keyword: "const",
                    params: { allowedValue: 1 },
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
              if (data.durableReceipts !== undefined) {
                const _errs6 = errors;
                if (typeof data.durableReceipts !== "boolean") {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/durableReceipts",
                      schemaPath: "#/properties/durableReceipts/type",
                      keyword: "type",
                      params: { type: "boolean" },
                      message: "must be boolean",
                    },
                  ];
                  return false;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.cursorAttach !== undefined) {
                  const _errs8 = errors;
                  if (typeof data.cursorAttach !== "boolean") {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/cursorAttach",
                        schemaPath: "#/properties/cursorAttach/type",
                        keyword: "type",
                        params: { type: "boolean" },
                        message: "must be boolean",
                      },
                    ];
                    return false;
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.a2ui !== undefined) {
                    let data4 = data.a2ui;
                    const _errs10 = errors;
                    const _errs11 = errors;
                    if (errors === _errs11) {
                      if (
                        data4 &&
                        typeof data4 == "object" &&
                        !Array.isArray(data4)
                      ) {
                        let missing1;
                        if (
                          (data4.version === undefined &&
                            (missing1 = "version")) ||
                          (data4.catalogId === undefined &&
                            (missing1 = "catalogId")) ||
                          (data4.catalogVersion === undefined &&
                            (missing1 = "catalogVersion"))
                        ) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/a2ui",
                              schemaPath: "#/$defs/A2uiNegotiation/required",
                              keyword: "required",
                              params: { missingProperty: missing1 },
                              message:
                                "must have required property '" +
                                missing1 +
                                "'",
                            },
                          ];
                          return false;
                        } else {
                          const _errs13 = errors;
                          for (const key1 in data4) {
                            if (
                              !(
                                key1 === "version" ||
                                key1 === "catalogId" ||
                                key1 === "catalogVersion"
                              )
                            ) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/a2ui",
                                  schemaPath:
                                    "#/$defs/A2uiNegotiation/additionalProperties",
                                  keyword: "additionalProperties",
                                  params: { additionalProperty: key1 },
                                  message:
                                    "must NOT have additional properties",
                                },
                              ];
                              return false;
                              break;
                            }
                          }
                          if (_errs13 === errors) {
                            if (data4.version !== undefined) {
                              let data5 = data4.version;
                              const _errs14 = errors;
                              if (typeof data5 !== "string") {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/a2ui/version",
                                    schemaPath:
                                      "#/$defs/A2uiNegotiation/properties/version/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                              if ("v0.9.1" !== data5) {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/a2ui/version",
                                    schemaPath:
                                      "#/$defs/A2uiNegotiation/properties/version/const",
                                    keyword: "const",
                                    params: { allowedValue: "v0.9.1" },
                                    message: "must be equal to constant",
                                  },
                                ];
                                return false;
                              }
                              var valid2 = _errs14 === errors;
                            } else {
                              var valid2 = true;
                            }
                            if (valid2) {
                              if (data4.catalogId !== undefined) {
                                let data6 = data4.catalogId;
                                const _errs16 = errors;
                                if (typeof data6 !== "string") {
                                  validate21.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/a2ui/catalogId",
                                      schemaPath:
                                        "#/$defs/A2uiNegotiation/properties/catalogId/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                                if (
                                  "urn:rss-mdm-agent:a2ui:interaction" !== data6
                                ) {
                                  validate21.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/a2ui/catalogId",
                                      schemaPath:
                                        "#/$defs/A2uiNegotiation/properties/catalogId/const",
                                      keyword: "const",
                                      params: {
                                        allowedValue:
                                          "urn:rss-mdm-agent:a2ui:interaction",
                                      },
                                      message: "must be equal to constant",
                                    },
                                  ];
                                  return false;
                                }
                                var valid2 = _errs16 === errors;
                              } else {
                                var valid2 = true;
                              }
                              if (valid2) {
                                if (data4.catalogVersion !== undefined) {
                                  let data7 = data4.catalogVersion;
                                  const _errs18 = errors;
                                  if (typeof data7 !== "string") {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/a2ui/catalogVersion",
                                        schemaPath:
                                          "#/$defs/A2uiNegotiation/properties/catalogVersion/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                  if ("1" !== data7) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/a2ui/catalogVersion",
                                        schemaPath:
                                          "#/$defs/A2uiNegotiation/properties/catalogVersion/const",
                                        keyword: "const",
                                        params: { allowedValue: "1" },
                                        message: "must be equal to constant",
                                      },
                                    ];
                                    return false;
                                  }
                                  var valid2 = _errs18 === errors;
                                } else {
                                  var valid2 = true;
                                }
                              }
                            }
                          }
                        }
                      } else {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/a2ui",
                            schemaPath: "#/$defs/A2uiNegotiation/type",
                            keyword: "type",
                            params: { type: "object" },
                            message: "must be object",
                          },
                        ];
                        return false;
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
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate20.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
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
  validate20.errors = vErrors;
  return errors === 0;
}
validate20.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
