import type {
  NativeRuntime,
  OperationHandlers,
} from "../../../packages/ai-adapters/deepseek/dist/protocol.js";
declare const runtime: NativeRuntime;
declare const budget: Parameters<NativeRuntime["call"]>[2];
const inspected = await runtime.call(
  "inspect",
  { requestId: "request" },
  budget,
);
if (inspected.status === "terminal") inspected.outcome;
// @ts-expect-error An inspect request cannot carry prompt-only data.
await runtime.call("inspect", { text: "wrong request" }, budget);
// @ts-expect-error Operation and result stay correlated.
const accepted: string = inspected.requestId;
const handlers: Pick<OperationHandlers, "prompt"> = {
  // @ts-expect-error Worker must return the prompt response, not an inspect result.
  prompt: async () => ({ status: "unknown" }),
};
void handlers;
