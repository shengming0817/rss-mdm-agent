import { startStage, providerStage } from "../contexts.js";
import { replaceStage } from "../contexts.js";
import { activeStage } from "../contexts.js";
import assert from "node:assert/strict";
import canonicalize from "canonicalize";
import { withinBudget, withCleanup, type BudgetFactory } from "./budget.js";
import type {
  Binding,
  Command,
  CommandRecord,
  DispatchAttempt,
} from "../wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderSessionBinding,
  ProviderObservation,
  Result,
  Submission,
  Reconciliation,
} from "../ports.js";
import { fixtureSession, fixtureCommand, unwrap } from "./conformance.js";
import { ok, fail, fixtureLimits } from "./store.js";
import { VerifiedProviderSession, workspaceIdentity } from "../session.js";
import { boundedJson, decode, fingerprint } from "../codec.js";
let nextProviderInstance = 0;
/** Scripted provider contract double. Never spawns a process or executes a tool. */
export class ScriptedProvider implements ProviderAgentPort {
  readonly evidence = "scripted_provider" as const;
  constructor(
    private readonly capabilities: Partial<
      import("../wire.js").Capabilities
    > = {},
  ) {}
  private configuration?: ProviderConfiguration;
  private binding = activeStage(fixtureSession()).binding;
  observations: ProviderObservation[] = [];
  submission: "submitted" | "unknown" | "not_sent" = "submitted";
  dispatched = 0;
  private readonly instance = ++nextProviderInstance;
  private incarnation = 0;
  private closed = false;
  async createSession(
    configuration: ProviderConfiguration,
    _budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    if (this.closed || _budget.signal.aborted) return fail("unavailable");
    if (
      configuration.provider !== "fake" ||
      configuration.permissions !== "tools_disabled"
    )
      return fail("unsupported_capability");
    this.configuration = configuration;
    this.binding = {
      ...activeStage(fixtureSession()).binding,
      workspaceId: workspaceIdentity(configuration.workingDirectory),
      generation: `generation-${this.instance}-${++this.incarnation}`,
      nativeSessionId: `native-${this.instance}-${this.incarnation}`,
      config: structuredClone(configuration.config),
      accountRef: configuration.accountRef,
    };
    return ok({
      binding: structuredClone(this.binding),
      capabilities: {
        ...activeStage(fixtureSession()).capabilities,
        ...this.capabilities,
        tools: "disabled",
      },
    });
  }
  async resume(
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    const opened = await this.createSession(configuration, budget);
    if (!opened.ok || this.closed || budget.signal.aborted)
      return fail("unavailable");
    this.binding = { ...binding, generation: opened.value.binding.generation };
    return ok({
      binding: structuredClone(this.binding),
      capabilities: {
        ...opened.value.capabilities,
        continuation: "across_processes",
      },
    });
  }
  async dispatch(
    binding: Binding,
    command: Command,
    attempt: DispatchAttempt,
    _budget: Budget,
  ): Promise<Submission> {
    if (
      !this.configuration ||
      canonicalize(binding) !== canonicalize(this.binding) ||
      attempt.observerGeneration !== binding.generation ||
      attempt.nativeSessionId !== binding.nativeSessionId ||
      attempt.nativeThreadId !== binding.nativeThreadId
    )
      return {
        certainty: "not_sent",
        error: { code: "stale_binding", retry: "never" },
      };
    if (this.closed)
      return {
        certainty: "not_sent",
        error: { code: "unavailable", retry: "never" },
      };
    if (command.input.type !== "prompt")
      return command.input.type === "cancel"
        ? {
            certainty: "acknowledged",
            binding,
            acknowledgement: { type: "cancel", confirmation: "request_only" },
          }
        : {
            certainty: "not_sent",
            error: { code: "unavailable", retry: "never" },
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
  async *observe(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    if (this.closed || canonicalize(binding) !== canonicalize(this.binding))
      return;
    for (const event of this.observations) {
      if (this.closed || budget.signal.aborted) return;
      if (canonicalize(event.binding) === canonicalize(binding))
        yield structuredClone(event);
    }
  }
  async reconcile(
    binding: Binding,
    record: CommandRecord,
    _budget: Budget,
  ): Promise<Result<Reconciliation>> {
    return !this.closed &&
      record.dispatch &&
      canonicalize(binding) === canonicalize(this.binding)
      ? ok({
          status: "unknown",
          commandId: record.command.commandId,
          attemptId: record.dispatch.attemptId,
          binding: structuredClone(binding),
        })
      : fail("stale_binding");
  }
  async close(_budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    this.closed = true;
    this.configuration = undefined;
    this.observations = [];
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
        const admitted = unwrap(
          await withinBudget(budget, (b) =>
            VerifiedProviderSession.open(port, configuration, b),
          ),
        );
        const { binding, capabilities } = admitted;
        assert.deepEqual(binding.config, configuration.config);
        assert.equal(binding.accountRef, configuration.accountRef);
        const command = fixtureCommand();
        const attempt: DispatchAttempt = {
          attemptId: "attempt-command-1",
          originGeneration: binding.generation,
          observerGeneration: binding.generation,
          nativeSessionId: binding.nativeSessionId,
          ...(binding.nativeThreadId
            ? { nativeThreadId: binding.nativeThreadId }
            : {}),
          certainty: "intent",
        };
        const submission = await withinBudget(budget, (b) =>
          port.dispatch(binding, command, attempt, b),
        );
        assert.equal(submission.certainty, scenario);
        if (scenario === "unknown") {
          assert.equal(submission.certainty, "unknown");
          if (submission.certainty !== "unknown")
            throw new Error("expected unknown");
          assert.ok(submission.correlationId);
          const record: CommandRecord = {
            schemaVersion: 5,
            kind: "commandRecord",
            command,
            receipt: {
              schemaVersion: 5,
              kind: "receipt",
              stageId: binding.generation,
              namespace: configuration.namespace,
              commandId: command.commandId,
              contentHash: fingerprint(command, fixtureLimits),
              acceptedAtMs: 0,
              retryUntilMs: 100,
              receiptUntilMs: 200,
              acceptedRevision: 1,
            },
            state: "reconciliation_required",
            dispatch: {
              ...attempt,
              correlationId: submission.correlationId,
              certainty: "unknown",
            },
          };
          const reconciled = unwrap(
            await withinBudget(budget, (b) =>
              admitted.reconcile(
                startStage(
                  {
                    ...fixtureSession(),
                    namespace: configuration.namespace,
                    stages: [],
                  },
                  providerStage(binding, capabilities),
                ),
                record,
                b,
              ),
            ),
          );
          assert.equal(reconciled.observation.status, "unknown");
          assert.equal(reconciled.observation.outcome, undefined);
          assert.deepEqual(reconciled.observation.binding, binding);
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
            port.dispatch(
              submission.binding,
              cancellation,
              { ...attempt, attemptId: "control-attempt" },
              b,
            ),
          );
          if (
            capabilities.cancellation === "unsupported" ||
            capabilities.cancellation === "unknown"
          )
            assert.equal(
              result.certainty === "not_sent" &&
                result.error.code === "unsupported_capability",
              true,
            );
          else
            assert.equal(
              result.certainty === "acknowledged" &&
                result.acknowledgement.type === "cancel" &&
                result.acknowledgement.confirmation,
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
                assert.equal(observation.attemptId, attempt.attemptId);
                assert.deepEqual(
                  observation.binding,
                  submission.certainty === "submitted"
                    ? submission.binding
                    : binding,
                );
                const context = {
                  schemaVersion: 5,
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
                } else if (
                  observation.type === "submitted" ||
                  observation.type === "running" ||
                  observation.type === "interaction_unavailable"
                ) {
                  decode(
                    boundedJson(
                      {
                        ...context,
                        kind: "event",
                        eventId: "native-lifecycle",
                        sequence: count,
                        attemptId: observation.attemptId,
                        body:
                          observation.type === "submitted" ||
                          observation.type === "running"
                            ? { type: "status", state: "running" }
                            : {
                                type: "interaction",
                                interactionId: observation.interactionId,
                                status: "unavailable",
                              },
                      },
                      fixtureLimits,
                    ),
                    fixtureLimits,
                  );
                } else {
                  if (observation.type === "event")
                    assert.ok(
                      [
                        "text",
                        "terminal",
                        "tool_proposal",
                        "tool_result",
                        "error",
                        "cancel_dispatched",
                        "surface",
                      ].includes(observation.body.type),
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
                        ...(observation.type === "event" &&
                        observation.body.type === "error"
                          ? {}
                          : { attemptId: attempt.attemptId }),
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
  await lateAdmission(create, configuration, budget);
}

/** Deferred adapter seam tests both initialization entry points and both sides of close. */
async function lateAdmission(
  create: (
    scenario: "submitted" | "unknown",
  ) => ProviderAgentPort | Promise<ProviderAgentPort>,
  configuration: ProviderConfiguration,
  budget: BudgetFactory,
) {
  for (const operation of ["createSession", "resume"] as const)
    for (const deferStart of [false, true]) {
      const port = await withinBudget(budget, () => create("unknown"));
      const initialize = port[operation]?.bind(port);
      if (!initialize) {
        unwrap(await port.close(budget()));
        continue;
      }
      let release!: () => void, entered!: () => void, settled!: () => void;
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      const started = new Promise<void>((resolve) => {
        entered = resolve;
      });
      const finished = new Promise<void>((resolve) => {
        settled = resolve;
      });
      let late: Result<ProviderSessionBinding> | undefined;
      const delayed = async (...args: unknown[]) => {
        try {
          entered();
          if (deferStart) await gate;
          late = await (
            initialize as (
              ...input: unknown[]
            ) => Promise<Result<ProviderSessionBinding>>
          )(...args);
          if (!deferStart) await gate;
          return late;
        } finally {
          settled();
        }
      };
      Object.assign(port, { [operation]: delayed });
      const control = new AbortController();
      let previous = {
        ...fixtureSession(),
        namespace: configuration.namespace,
      };
      if (operation === "resume") {
        const original = await withinBudget(budget, () => create("unknown"));
        await withCleanup(
          async () => {
            const admitted = unwrap(
              await VerifiedProviderSession.open(
                original,
                configuration,
                budget(),
              ),
            );
            previous = replaceStage(
              { ...previous },
              admitted.binding,
              admitted.capabilities,
            );
          },
          async () => {
            unwrap(await withinBudget(budget, (b) => original.close(b)));
          },
        );
      }
      await withCleanup(
        async () => {
          const admission =
            operation === "resume"
              ? VerifiedProviderSession.restore(port, previous, configuration, {
                  ...budget(),
                  signal: control.signal,
                })
              : VerifiedProviderSession.open(port, configuration, {
                  ...budget(),
                  signal: control.signal,
                });
          await withinBudget(budget, () => started);
          control.abort();
          const result = await withinBudget(budget, () => admission);
          assert.equal(result.ok, false);
          if (!result.ok) assert.equal(result.cleanupError, undefined);
          release();
          await withinBudget(budget, () => finished);
          unwrap(await withinBudget(budget, (b) => port.close(b)));
          assert.equal(
            (
              await withinBudget(budget, (b) =>
                operation === "resume"
                  ? (initialize as NonNullable<ProviderAgentPort["resume"]>)(
                      activeStage(previous).binding,
                      configuration,
                      b,
                    )
                  : (initialize as ProviderAgentPort["createSession"])(
                      configuration,
                      b,
                    ),
              )
            ).ok,
            false,
            "closed adapter cannot admit a late creation/resume",
          );
          if (late?.ok) {
            const binding = late.value.binding;
            assert.equal(
              (
                await withinBudget(budget, (b) =>
                  port.dispatch(
                    binding,
                    fixtureCommand(),
                    {
                      attemptId: "late",
                      originGeneration: binding.generation,
                      observerGeneration: binding.generation,
                      nativeSessionId: binding.nativeSessionId,
                      certainty: "intent",
                    },
                    b,
                  ),
                )
              ).certainty,
              "not_sent",
              "late binding cannot operate after close",
            );
            const iterator = port
              .observe(binding, budget())
              [Symbol.asyncIterator]();
            try {
              assert.equal(
                (await withinBudget(budget, () => iterator.next())).done,
                true,
              );
            } finally {
              await withinBudget(budget, () => iterator.return?.());
            }
          }
        },
        async () => {
          release();
          unwrap(await withinBudget(budget, (b) => port.close(b)));
        },
      );
    }
}
