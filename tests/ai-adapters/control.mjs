import assert from "node:assert/strict";
/** Control test driver: verifies the dispatch acknowledgement separately from model outcomes. */
export async function acknowledge(provider, binding, command, budget) {
  const result = await provider.dispatch(
    binding,
    command,
    {
      attemptId: `control-${command.commandId}`,
      originGeneration: binding.generation,
      observerGeneration: binding.generation,
      nativeSessionId: binding.nativeSessionId,
      ...(binding.nativeThreadId
        ? { nativeThreadId: binding.nativeThreadId }
        : {}),
      ...(binding.nativeRunId ? { nativeRunId: binding.nativeRunId } : {}),
      ...(binding.nativeRequestId
        ? { nativeRequestId: binding.nativeRequestId }
        : {}),
      certainty: "intent",
    },
    budget,
  );
  if (result.certainty === "not_sent")
    return { ok: false, error: result.error };
  if (result.certainty === "unknown")
    return {
      ok: false,
      error: { code: "unavailable", retry: "reconcile_first" },
    };
  assert.equal(result.certainty, "acknowledged");
  assert.equal(result.acknowledgement.type, command.input.type);
  return { ok: true, value: result.acknowledgement.confirmation };
}
