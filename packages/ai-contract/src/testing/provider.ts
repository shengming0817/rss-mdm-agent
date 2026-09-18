import assert from "node:assert/strict";
import type { Binding, Capabilities, Command, CommandRecord } from "../wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderObservation,
  Result,
  Submission,
} from "../ports.js";
import { fixtureSession, fixtureCommand, unwrap } from "./conformance.js";
import { ok, fail, fixtureLimits } from "./store.js";
import { fingerprint } from "../codec.js";
/** Scripted provider contract double. Never spawns a process or executes a tool. */
export class ScriptedProvider implements ProviderAgentPort {
  readonly evidence = "scripted_provider" as const;
  private configuration?: ProviderConfiguration;
  private binding = fixtureSession().binding;
  observations: ProviderObservation[] = [];
  submission: "submitted" | "unknown" | "not_sent" = "submitted";
  dispatched = 0;
  async initialize(
    configuration: ProviderConfiguration,
    _budget: Budget,
  ): Promise<Result<Capabilities>> {
    if (configuration.permissions !== "tools_disabled")
      return fail("unsupported_capability");
    this.configuration = configuration;
    this.binding = {
      ...this.binding,
      config: configuration.config,
      accountRef: configuration.accountRef,
    };
    return ok(fixtureSession().capabilities);
  }
  async createSession(_budget: Budget): Promise<Result<Binding>> {
    return this.configuration
      ? ok(structuredClone(this.binding))
      : fail("unavailable");
  }
  async submit(
    binding: Binding,
    command: Command,
    _budget: Budget,
  ): Promise<Submission> {
    if (
      !this.configuration ||
      JSON.stringify(binding) !== JSON.stringify(this.binding) ||
      command.input.type !== "prompt"
    )
      return {
        certainty: "not_sent",
        error: { code: "stale_binding", retry: "never" },
      };
    this.dispatched++;
    if (this.submission === "unknown")
      return { certainty: "unknown", correlationId: command.commandId };
    if (this.submission === "not_sent")
      return {
        certainty: "not_sent",
        error: { code: "unavailable", retry: "same_command" },
      };
    this.binding = {
      ...binding,
      nativeRunId: `run-${command.commandId}`,
      nativeRequestId: `request-${command.commandId}`,
    };
    return { certainty: "submitted", binding: structuredClone(this.binding) };
  }
  async cancel(
    binding: Binding,
    _command: Command,
    _budget: Budget,
  ): Promise<Result<"request_only">> {
    return binding.generation === this.binding.generation
      ? ok("request_only")
      : fail("stale_binding");
  }
  async respond(
    _binding: Binding,
    _command: Command,
    _budget: Budget,
  ): Promise<Result<void>> {
    return fail("unavailable");
  }
  async *observe(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    for (const event of this.observations) {
      if (budget.signal.aborted) return;
      if (event.binding.generation === binding.generation)
        yield structuredClone(event);
    }
  }
  async reconcile(
    binding: Binding,
    _record: CommandRecord,
    _budget: Budget,
  ): Promise<Result<{ status: "unknown"; binding: Binding }>> {
    return ok({ status: "unknown", binding: structuredClone(binding) });
  }
  async close(_budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    this.configuration = undefined;
    return ok({ processStopped: true });
  }
}
/** A deterministic adapter harness supplies both a submitted request and an ambiguous
 * dispatch, with no actual terminal observation. Its scripted observation stream ends.
 * Real process/crash/tool containment evidence remains adapter-owned. */
export async function runProviderConformance(
  create: (
    scenario: "submitted" | "unknown",
  ) => ProviderAgentPort | Promise<ProviderAgentPort>,
  configuration: ProviderConfiguration,
  budget: Budget,
): Promise<void> {
  for (const scenario of ["submitted", "unknown"] as const) {
    const port = await create(scenario);
    let failure: unknown;
    try {
      const capabilities = unwrap(await port.initialize(configuration, budget));
      const binding = unwrap(await port.createSession(budget));
      assert.deepEqual(binding.config, configuration.config);
      assert.equal(binding.accountRef, configuration.accountRef);
      const command = fixtureCommand();
      const submission = await port.submit(binding, command, budget);
      assert.equal(submission.certainty, scenario);
      if (scenario === "unknown") {
        assert.equal(submission.certainty, "unknown");
        if (submission.certainty !== "unknown")
          throw new Error("expected unknown");
        assert.ok(submission.correlationId);
        const record: CommandRecord = {
          schemaVersion: 2,
          kind: "commandRecord",
          command,
          receipt: {
            schemaVersion: 2,
            kind: "receipt",
            namespace: fixtureSession().namespace,
            commandId: command.commandId,
            contentHash: fingerprint(command, fixtureLimits),
            acceptedAtMs: 0,
            retryUntilMs: 100,
            receiptUntilMs: 200,
            acceptedRevision: 1,
          },
          state: "reconciliation_required",
          dispatch: {
            generation: binding.generation,
            nativeSessionId: binding.nativeSessionId,
            certainty: "unknown",
          },
        };
        const reconciled = unwrap(
          await port.reconcile(binding, record, budget),
        );
        assert.equal(reconciled.status, "unknown");
        assert.equal(reconciled.outcome, undefined);
        assert.deepEqual(reconciled.binding, binding);
      } else {
        if (submission.certainty !== "submitted")
          throw new Error("expected submission");
        const cancellation: Command = {
          ...command,
          commandId: "cancel-1",
          input: {
            type: "cancel",
            targetCommandId: command.commandId,
            generation: submission.binding.generation,
            ...(submission.binding.nativeRunId
              ? { nativeRunId: submission.binding.nativeRunId }
              : {}),
          },
        };
        const result = await port.cancel(
          submission.binding,
          cancellation,
          budget,
        );
        if (
          capabilities.cancellation === "unsupported" ||
          capabilities.cancellation === "unknown"
        )
          assert.equal(
            result.ok
              ? result.value === "unsupported"
              : result.error.code === "unsupported_capability",
            true,
          );
        else
          assert.equal(
            unwrap(result),
            "request_only",
            "a cancel request cannot acknowledge an unobserved terminal",
          );
      }
      for await (const observation of port.observe(
        submission.certainty === "submitted" ? submission.binding : binding,
        budget,
      )) {
        assert.equal(observation.commandId, command.commandId);
        assert.equal(observation.binding.generation, binding.generation);
        assert.equal(
          observation.type === "event" && observation.body.type === "terminal",
          false,
          "fixture has no native terminal",
        );
      }
    } catch (error) {
      failure = error;
    } finally {
      try {
        const cleanup = { timeoutMs: 1000, signal: AbortSignal.timeout(1000) };
        assert.equal(unwrap(await port.close(cleanup)).processStopped, true);
      } catch (error) {
        failure = failure
          ? new AggregateError(
              [failure, error],
              "provider conformance and cleanup failed",
            )
          : error;
      }
    }
    if (failure) throw failure;
  }
}
