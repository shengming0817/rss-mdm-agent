import type {
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderAdmission,
  Subscription,
  ProviderInteraction,
  ProviderObservation,
  ProviderEventBody,
} from "../../packages/ai-contract/dist/index.js";
import { VerifiedProviderSession } from "../../packages/ai-contract/dist/session.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
// @ts-expect-error Resume must return capabilities with binding.
const invalidResume: NonNullable<ProviderAgentPort["resume"]> = async () => ({
  ok: true,
  value: {} as import("../../packages/ai-contract/dist/index.js").Binding,
});
const controlledConfiguration: ProviderConfiguration = {
  provider: "fake",
  config: { id: "c", revision: "1" },
  namespace: fixtureSession().namespace,
  workingDirectory: ".",
  permissions: "host_mediated",
};
// @ts-expect-error Parent admission cannot omit its verifier or ToolEndpoint.
const invalidAdmission: ProviderAdmission = {};
// @ts-expect-error Admission cannot be built from serialized fields.
const forged: VerifiedProviderSession = { binding: {}, capabilities: {} };
// @ts-expect-error Every subscription delta retains its message identity.
const missingMessage: Subscription = {
  type: "delta",
  commandId: "c",
  generation: "g",
  text: "x",
};
const question: ProviderInteraction = {
  category: "question",
  interactionId: "question",
  nativeCallbackId: "callback",
  expiresAtMs: 1,
  callbackLifetime: "generation_bound",
  request: { question: "Choose" },
};
const oldQuestion: ProviderInteraction = {
  category: "question",
  interactionId: "question",
  nativeCallbackId: "callback",
  // @ts-expect-error Legacy parent request field is forbidden even with a valid callback.
  nativeRequestId: "request",
  expiresAtMs: 1,
  callbackLifetime: "generation_bound",
  request: {},
};
const permission: ProviderInteraction = {
  ...question,
  // @ts-expect-error Permission callbacks cannot enter the ordinary response lifecycle.
  category: "tool_permission",
};
const pending: ProviderObservation = {
  type: "event",
  binding: {} as any,
  commandId: "c",
  body: {
    // @ts-expect-error Initial pending callbacks cannot bypass their dedicated observation.
    type: "interaction",
    interactionId: "q",
    status: "pending",
    request: {},
  },
};
// @ts-expect-error Provider cannot manufacture a host/store lifecycle event.
const internalEvent: ProviderEventBody = { type: "session_retired" };
