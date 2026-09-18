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
import { ok, fail } from "./store.js";
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
/** Minimum lifecycle/submit/cancel contract. The caller supplies an isolated adapter
 * configuration and bounded signal; tool containment is a separate evidence profile. */
export async function runProviderConformance(
  port: ProviderAgentPort,
  configuration: ProviderConfiguration,
  budget: Budget,
): Promise<void> {
  const capabilities = unwrap(await port.initialize(configuration, budget));
  const binding = unwrap(await port.createSession(budget));
  assert.deepEqual(binding.config, configuration.config);
  assert.equal(binding.accountRef, configuration.accountRef);
  const command = fixtureCommand();
  const submission = await port.submit(binding, command, budget);
  assert.equal(submission.certainty, "submitted");
  if (submission.certainty !== "submitted")
    throw new Error("provider submit failed");
  const cancellation = {
    ...command,
    commandId: "cancel-1",
    input: {
      type: "cancel" as const,
      targetCommandId: command.commandId,
      generation: submission.binding.generation,
      nativeRunId: submission.binding.nativeRunId,
    },
  };
  const result = await port.cancel(submission.binding, cancellation, budget);
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
  else assert.equal(result.ok, true);
  unwrap(await port.close(budget));
}
