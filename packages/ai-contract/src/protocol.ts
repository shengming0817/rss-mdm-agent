import { readFileSync } from "node:fs";
import { Ajv2020 } from "ajv/dist/2020.js";
import { decode, boundedJson, type Limits } from "./codec.js";
import type {
  Caller,
  Result,
  SessionStore,
  ProviderObservation,
  Subscription,
} from "./ports.js";
import type { SurfaceAction, SurfaceBinding } from "./wire.js";
const actionSchema = JSON.parse(
  readFileSync(
    new URL("../schema/upstream/a2ui/client_to_server.json", import.meta.url),
    "utf8",
  ),
);
const validAction = new Ajv2020({
  strict: false,
  validateFormats: false,
}).compile(actionSchema);
/** ACP extension namespace. Advertise through capabilities._meta before calling methods. */
export const extension = {
  capability: "rss-mdm-agent.ai-runtime",
  submit: "_rss-mdm-agent/submit",
  snapshot: "_rss-mdm-agent/snapshot",
  attach: "_rss-mdm-agent/attach",
  update: "_rss-mdm-agent/update",
} as const;
/** Product association only; the upstream action context remains untrusted tool data.
 * Timestamp is an upstream annotation, never an expiry/authority source. The access
 * adapter owns transport byte limits and official renderer/catalog validation. */
export async function resolveSurfaceAction(
  store: SessionStore,
  caller: Caller,
  metadata: SurfaceAction,
  standard: unknown,
  limits: Limits,
): Promise<
  Result<{
    interactionId: string;
    commandId: string;
    answer: Record<string, unknown>;
    surface: { instanceId: string; revision: number };
  }>
> {
  const fail = (code: import("./wire.js").ErrorCode): Result<never> => ({
    ok: false,
    error: { code, retry: "never" },
  });
  let binding: SurfaceBinding;
  try {
    const checkedMetadata = decode(boundedJson(metadata, limits), limits);
    if (checkedMetadata.kind !== "surfaceAction") return fail("invalid_input");
    metadata = checkedMetadata;
    standard = JSON.parse(boundedJson(standard, limits));
  } catch {
    return fail("invalid_input");
  }
  const current = await store.surface(
    { ...caller, sessionId: metadata.sessionId },
    metadata.surfaceInstanceId,
  );
  if (!current.ok) return current;
  try {
    const checked = decode(boundedJson(current.value, limits), limits);
    if (checked.kind !== "surface") return fail("invalid_input");
    binding = checked;
  } catch {
    return fail("invalid_input");
  }
  if (binding.status !== "active") return fail("unavailable");
  if (!validAction(standard)) return fail("invalid_input");
  const message = standard as {
    version: string;
    action?: {
      name: string;
      surfaceId: string;
      sourceComponentId: string;
      context: Record<string, unknown>;
    };
  };
  const action = message.action;
  if (message.version !== "v0.9.1" || !action)
    return fail("unsupported_version");
  if (
    caller.tenantId !== binding.namespace.tenantId ||
    caller.principalId !== binding.namespace.principalId ||
    caller.authorityId !== binding.namespace.authorityId ||
    metadata.sessionId !== binding.namespace.sessionId
  )
    return fail("permission_denied");
  if (
    metadata.surfaceInstanceId !== binding.surfaceInstanceId ||
    metadata.surfaceRevision !== binding.revision ||
    metadata.interactionId !== binding.interactionId ||
    metadata.generation !== binding.generation ||
    metadata.nativeRunId !== binding.nativeRunId ||
    action.surfaceId !== binding.surfaceId ||
    action.sourceComponentId !== binding.sourceComponentId ||
    action.name !== binding.eventName
  )
    return fail("stale_binding");
  // Apply the same product budgets to the answer before returning it to Host.respond.
  try {
    decode(
      boundedJson(
        {
          schemaVersion: 2,
          kind: "command",
          sessionId: metadata.sessionId,
          commandId: metadata.commandId,
          expiresAtMs: 0,
          input: {
            type: "respond",
            interactionId: binding.interactionId,
            generation: binding.generation,
            nativeRunId: binding.nativeRunId,
            answer: action.context,
          },
        },
        limits,
      ),
      limits,
    );
  } catch {
    return fail("invalid_input");
  }
  return {
    ok: true,
    value: {
      interactionId: binding.interactionId,
      commandId: metadata.commandId,
      answer: structuredClone(action.context),
      surface: {
        instanceId: binding.surfaceInstanceId,
        revision: binding.revision,
      },
    },
  };
}

/** Projection only; the Host verifies provider binding/command association before publishing. */
export function projectDelta(
  observation: Extract<ProviderObservation, { type: "delta" }>,
): Extract<Subscription, { type: "delta" }> {
  return {
    type: "delta",
    generation: observation.binding.generation,
    commandId: observation.commandId,
    messageId: observation.messageId,
    text: observation.text,
  };
}
