import assert from "node:assert/strict";
import canonicalize from "canonicalize";
import { withinBudget, withCleanup, type BudgetFactory } from "./budget.js";
import type { Binding, Command, CommandRecord } from "../wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderSessionBinding,
  ProviderObservation,
  Result,
  Submission,
} from "../ports.js";
import { fixtureSession, fixtureCommand, unwrap } from "./conformance.js";
import { ok, fail, fixtureLimits } from "./store.js";
import { VerifiedProviderSession } from "../session.js";
import { boundedJson, decode, fingerprint } from "../codec.js";
let nextProviderInstance = 0;
/** Scripted provider contract double. Never spawns a process or executes a tool. */
export class ScriptedProvider implements ProviderAgentPort {
  readonly evidence = "scripted_provider" as const;
  private configuration?: ProviderConfiguration;
  private binding = fixtureSession().binding;
  observations: ProviderObservation[] = [];
  submission: "submitted" | "unknown" | "not_sent" = "submitted";
  dispatched = 0;
  private readonly instance = ++nextProviderInstance;
  private incarnation = 0;
  async createSession(
    configuration: ProviderConfiguration,
    _budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    if (
      configuration.provider !== "fake" ||
      configuration.permissions !== "tools_disabled"
    )
      return fail("unsupported_capability");
    this.configuration = configuration;
    this.binding = {
      ...fixtureSession().binding,
      generation: `generation-${this.instance}-${++this.incarnation}`,
      nativeSessionId: `native-${this.instance}-${this.incarnation}`,
      config: structuredClone(configuration.config),
      accountRef: configuration.accountRef,
    };
    return ok({
      binding: structuredClone(this.binding),
      capabilities: fixtureSession().capabilities,
    });
  }
  async submit(
    binding: Binding,
    command: Command,
    _budget: Budget,
  ): Promise<Submission> {
    if (
      !this.configuration ||
      canonicalize(binding) !== canonicalize(this.binding) ||
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
    return canonicalize(binding) === canonicalize(this.binding)
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
    if (canonicalize(binding) !== canonicalize(this.binding)) return;
    for (const event of this.observations) {
      if (budget.signal.aborted) return;
      if (canonicalize(event.binding) === canonicalize(binding))
        yield structuredClone(event);
    }
  }
  async reconcile(
    binding: Binding,
    _record: CommandRecord,
    _budget: Budget,
  ): Promise<Result<{ status: "unknown"; binding: Binding }>> {
    return canonicalize(binding) === canonicalize(this.binding)
      ? ok({ status: "unknown", binding: structuredClone(binding) })
      : fail("stale_binding");
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
  budget: BudgetFactory,
): Promise<void> {
  for (const scenario of ["submitted", "unknown"] as const) {
    const port = await withinBudget(budget, () => create(scenario));
    await withCleanup(
      async () => {
        const { binding, capabilities } = unwrap(
          await withinBudget(budget, (b) =>
            VerifiedProviderSession.open(port, configuration, b),
          ),
        );
        assert.deepEqual(binding.config, configuration.config);
        assert.equal(binding.accountRef, configuration.accountRef);
        const command = fixtureCommand();
        const submission = await withinBudget(budget, (b) =>
          port.submit(binding, command, b),
        );
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
            await withinBudget(budget, (b) =>
              port.reconcile(binding, record, b),
            ),
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
          const result = await withinBudget(budget, (b) =>
            port.cancel(submission.binding, cancellation, b),
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
        let iterator: AsyncIterator<ProviderObservation> | undefined;
        await withCleanup(
          () =>
            withinBudget(budget, async (b) => {
              iterator = port
                .observe(
                  submission.certainty === "submitted"
                    ? submission.binding
                    : binding,
                  b,
                )
                [Symbol.asyncIterator]();
              let count = 0;
              while (true) {
                const item: IteratorResult<ProviderObservation> =
                  await iterator.next();
                if (item.done) break;
                assert.ok(++count <= 1024, "bounded observation count");
                // Check the entire untrusted envelope before projecting it into wire.
                const observation: ProviderObservation = JSON.parse(
                  boundedJson(item.value, fixtureLimits),
                );
                assert.equal(observation.commandId, command.commandId);
                assert.deepEqual(
                  observation.binding,
                  submission.certainty === "submitted"
                    ? submission.binding
                    : binding,
                );
                const context = {
                  schemaVersion: 2,
                  namespace: fixtureSession().namespace,
                  commandId: command.commandId,
                  generation: observation.binding.generation,
                };
                if (observation.type === "interaction") {
                  decode(
                    boundedJson(
                      {
                        ...observation.interaction,
                        ...context,
                        kind: "interaction",
                        status: "pending",
                        ...(observation.binding.nativeRunId
                          ? { nativeRunId: observation.binding.nativeRunId }
                          : {}),
                      },
                      fixtureLimits,
                    ),
                    fixtureLimits,
                  );
                  assert.deepEqual(
                    Object.keys(observation.interaction).sort(),
                    [
                      "callbackLifetime",
                      "category",
                      "expiresAtMs",
                      "interactionId",
                      "nativeCallbackId",
                      "request",
                    ],
                  );
                } else {
                  assert.equal(
                    observation.type === "event" &&
                      observation.body.type === "interaction" &&
                      String(observation.body.status) === "pending",
                    false,
                    "pending callbacks require the dedicated question observation",
                  );
                  assert.ok(
                    observation.type === "event" ||
                      observation.type === "delta",
                  );
                  decode(
                    boundedJson(
                      {
                        ...context,
                        kind: "event",
                        eventId: "fixture-event",
                        sequence: count,
                        body:
                          observation.type === "event"
                            ? observation.body
                            : {
                                type: "text",
                                messageId: observation.messageId,
                                text: observation.text,
                              },
                      },
                      fixtureLimits,
                    ),
                    fixtureLimits,
                  );
                }
                assert.equal(
                  observation.type === "event" &&
                    observation.body.type === "terminal",
                  false,
                  "fixture has no native terminal",
                );
              }
            }),
          async () => {
            await withinBudget(budget, () => iterator?.return?.());
          },
        );
      },
      async () => {
        assert.equal(
          unwrap(await withinBudget(budget, (b) => port.close(b)))
            .processStopped,
          true,
        );
      },
    );
  }
}
