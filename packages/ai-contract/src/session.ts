import { createHash } from "node:crypto";
import { resolve } from "node:path";
import canonicalize from "canonicalize";
import { withinBudget } from "./budget.js";
import { boundedJson, decode, isId } from "./codec.js";
import type {
  Binding,
  Capabilities,
  Session,
  Failure,
  Namespace,
  CommandRecord,
  Id,
} from "./wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  Result,
  ToolEndpoint,
  Reconciliation,
  ProviderInstance,
  ProviderForkPort,
  ProviderForkRequest,
  ProviderForkResult,
} from "./ports.js";

/** Logical workspace path identity, never a filesystem containment proof. */
export function workspaceIdentity(directory: string): string {
  if (
    typeof directory !== "string" ||
    !directory ||
    directory.length > 32768 ||
    directory.includes("\0")
  )
    throw new TypeError("invalid workspace");
  return createHash("sha256").update(resolve(directory)).digest("hex");
}

export type AdmissionResult =
  | { ok: true; value: VerifiedProviderSession }
  | { ok: false; error: Failure; cleanupError?: Failure };
export type ForkAdmissionResult =
  | {
      certainty: "created";
      session: VerifiedProviderSession;
      source: ProviderForkRequest;
    }
  | {
      certainty: "not_created" | "unknown";
      error: Failure;
      cleanupError?: Failure;
    };
interface ForkAdmission {
  port?: ProviderForkPort;
  rejection?: Failure;
  request: ProviderForkRequest;
  result?: ProviderForkResult;
  attempted: boolean;
}
const usedPorts = new WeakSet<ProviderAgentPort>();
const unavailable = (): Failure => ({
  code: "unavailable",
  retry: "same_command",
});
const authority = Symbol("verified provider session");
const denied = (): Result<never> => ({
  ok: false,
  error: { code: "permission_denied", retry: "never" },
});
const same = (a: unknown, b: unknown) => canonicalize(a) === canonicalize(b);

/** Stable observer identity; run/request coordinates may advance within it. */
export const providerIdentity = ({
  nativeRunId: _run,
  nativeRequestId: _request,
  ...identity
}: Binding) => identity;
const proofBrand: unique symbol = Symbol("verified reconciliation");
/** Process-local evidence minted only by an admitted provider call. */
export interface VerifiedReconciliation {
  readonly [proofBrand]: true;
  readonly commandId: Id;
  readonly observation: Reconciliation;
}
const observations = new WeakMap<
  VerifiedReconciliation,
  {
    session: Pick<Session, "namespace" | "binding">;
    record: CommandRecord;
    observation: Reconciliation;
  }
>();
/** Internal transition check; structural copies cannot recover the private evidence. */
export function reconciliationFor(
  proof: VerifiedReconciliation,
  session: Session,
  record: CommandRecord,
): Reconciliation | undefined {
  const evidence = observations.get(proof);
  return evidence &&
    same(evidence.session, {
      namespace: session.namespace,
      binding: session.binding,
    }) &&
    same(evidence.record, record)
    ? structuredClone(evidence.observation)
    : undefined;
}

/** Nominal, process-local admission evidence. JSON and structural casts cannot mint it.
 * The injected verifier is trusted code owned by the composition root; its actual
 * platform containment proof remains adapter-owned. */
export class VerifiedProviderSession {
  readonly #binding: Binding;
  readonly #namespace: Namespace;
  readonly #port: ProviderAgentPort;
  readonly #capabilities: Capabilities;
  readonly #tools?: ToolEndpoint;
  readonly #previous?: Pick<Session, "namespace" | "binding">;
  private constructor(
    token: symbol,
    port: ProviderAgentPort,
    namespace: Namespace,
    binding: Binding,
    capabilities: Capabilities,
    tools?: ToolEndpoint,
    previous?: Pick<Session, "namespace" | "binding">,
  ) {
    if (token !== authority) throw new TypeError("unverified provider session");
    this.#binding = structuredClone(binding);
    this.#namespace = structuredClone(namespace);
    this.#port = port;
    this.#capabilities = structuredClone(capabilities);
    this.#tools = tools;
    this.#previous = previous && structuredClone(previous);
    Object.freeze(this);
  }
  get binding(): Binding {
    return structuredClone(this.#binding);
  }
  get capabilities(): Capabilities {
    return structuredClone(this.#capabilities);
  }
  matches(binding: Binding, tools?: ToolEndpoint): boolean {
    return same(this.#binding, binding) && this.#tools === tools;
  }
  /** Only the resume path can establish this link. */
  restores(previous: Session): boolean {
    return (
      this.#previous !== undefined &&
      same(
        { namespace: previous.namespace, binding: previous.binding },
        this.#previous,
      )
    );
  }
  /** Reconcile the exact stored attempt through this admitted instance. No caller-supplied observation is accepted. */
  async reconcile(
    session: Session,
    record: CommandRecord,
    budget: Budget,
  ): Promise<Result<VerifiedReconciliation>> {
    try {
      const head = structuredClone(session),
        original = structuredClone(record);
      const limits = {
        maxBytes: 262144,
        maxTextBytes: 131072,
        maxDepth: 32,
        maxNodes: 16384,
      };
      decode(boundedJson(head, limits), limits);
      decode(boundedJson(original, limits), limits);
      if (
        head.status !== "active" ||
        !original.dispatch ||
        ["terminal", "invalidated"].includes(original.state) ||
        !same(head.namespace, this.#namespace) ||
        !same(original.receipt.namespace, this.#namespace) ||
        original.command.sessionId !== this.#namespace.sessionId ||
        !same(
          providerIdentity(head.binding),
          providerIdentity(this.#binding),
        ) ||
        original.dispatch.observerGeneration !== this.#binding.generation ||
        original.dispatch.nativeSessionId !== this.#binding.nativeSessionId ||
        original.dispatch.nativeThreadId !== this.#binding.nativeThreadId
      )
        return denied();
      return await withinBudget(
        () => budget,
        async (b) => {
          const response = await this.#port.reconcile(
            structuredClone(head.binding),
            structuredClone(original),
            b,
          );
          if (!response.ok) return response;
          const observed: Reconciliation = JSON.parse(
            boundedJson(response.value, limits),
          );
          const keys = [
            "attemptId",
            "binding",
            "commandId",
            "status",
            ...(observed.status === "terminal" ? ["outcome"] : []),
          ].sort();
          if (
            b.signal.aborted ||
            !same(Object.keys(observed).sort(), keys) ||
            observed.commandId !== original.command.commandId ||
            observed.attemptId !== original.dispatch!.attemptId ||
            !["unknown", "running", "terminal", "not_submitted"].includes(
              observed.status,
            ) ||
            !same(
              providerIdentity(observed.binding),
              providerIdentity(head.binding),
            ) ||
            (["nativeRunId", "nativeRequestId"] as const).some(
              (k) =>
                original.dispatch![k] !== undefined &&
                observed.binding[k] !== original.dispatch![k],
            )
          )
            return denied();
          decode(
            boundedJson({ ...head, binding: observed.binding }, limits),
            limits,
          );
          if (observed.status === "terminal")
            decode(
              boundedJson(
                {
                  ...original,
                  state: "terminal",
                  dispatch: { ...original.dispatch, certainty: "submitted" },
                  outcome: observed.outcome,
                },
                limits,
              ),
              limits,
            );
          const proof: VerifiedReconciliation = Object.freeze({
            [proofBrand]: true as const,
            commandId: observed.commandId,
            get observation() {
              return structuredClone(observed);
            },
          });
          observations.set(proof, {
            session: { namespace: head.namespace, binding: head.binding },
            record: original,
            observation: observed,
          });
          return { ok: true as const, value: proof };
        },
      );
    } catch {
      return { ok: false, error: unavailable() };
    }
  }
  /** Host calls this with its already-owned fresh child; admission and cleanup stay here.
   * The Host persists the successful child and lineage before publishing it to clients. */
  async fork(
    child: ProviderInstance,
    throughTurnId: Id,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<ForkAdmissionResult> {
    const request: ProviderForkRequest = {
      namespace: structuredClone(this.#namespace),
      binding: this.binding,
      throughTurnId,
    };
    if (child.agent === this.#port || usedPorts.has(child.agent))
      return {
        certainty: "not_created",
        error: { code: "permission_denied", retry: "never" },
      };
    const fork: ForkAdmission = {
      port: child.extensions.fork,
      request,
      attempted: false,
    };
    try {
      const { sessionId: parentId, ...parentScope } = this.#namespace;
      const { sessionId: childId, ...childScope } = configuration.namespace;
      if (this.#capabilities.fork !== "supported" || !fork.port)
        fork.rejection = { code: "unsupported_capability", retry: "never" };
      else if (
        !isId(throughTurnId) ||
        parentId === childId ||
        !same(parentScope, childScope) ||
        configuration.provider !== this.#binding.provider ||
        configuration.accountRef !== this.#binding.accountRef ||
        !same(configuration.config, this.#binding.config) ||
        workspaceIdentity(configuration.workingDirectory) !==
          this.#binding.workspaceId ||
        (configuration.permissions === "host_mediated"
          ? "host_mediated"
          : "disabled") !== this.#capabilities.tools
      )
        fork.rejection = { code: "permission_denied", retry: "never" };
    } catch {
      fork.rejection = { code: "invalid_input", retry: "never" };
    }
    // Rejection also consumes and cleans up the fresh Host-owned child.
    const admitted = await VerifiedProviderSession.admit(
      child.agent,
      configuration,
      budget,
      undefined,
      fork,
    );
    if (admitted.ok)
      return {
        certainty: "created",
        session: admitted.value,
        source: structuredClone(request),
      };
    return {
      certainty:
        fork.result?.certainty === "not_created" || !fork.attempted
          ? "not_created"
          : "unknown",
      error: admitted.error,
      ...(admitted.cleanupError ? { cleanupError: admitted.cleanupError } : {}),
    };
  }
  static restore(
    port: ProviderAgentPort,
    previous: Session,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<AdmissionResult> {
    return this.admit(port, configuration, budget, previous);
  }
  static open(
    port: ProviderAgentPort,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<AdmissionResult> {
    return this.admit(port, configuration, budget);
  }
  private static async admit(
    port: ProviderAgentPort,
    configuration: ProviderConfiguration,
    budget: Budget,
    previous?: Session,
    fork?: ForkAdmission,
  ): Promise<AdmissionResult> {
    // Each instance belongs to one admission. Refusing a second call never closes
    // the first successful session, including simultaneous calls.
    if (usedPorts.has(port) || typeof port.close !== "function")
      return denied();
    usedPorts.add(port);
    let result: Result<VerifiedProviderSession>;
    try {
      const prior = previous && structuredClone(previous);
      const settings = {
        ...configuration,
        namespace: structuredClone(configuration.namespace),
        workingDirectory: resolve(configuration.workingDirectory),
        config: structuredClone(configuration.config),
      };
      result = await withinBudget(
        () => budget,
        (b) => this.initialize(port, settings, b, prior, fork),
      );
    } catch {
      result = { ok: false, error: unavailable() };
    }
    if (result.ok) return result;
    try {
      const closed = await withinBudget(
        () => ({
          timeoutMs:
            Number.isSafeInteger(budget.timeoutMs) && budget.timeoutMs > 0
              ? Math.min(budget.timeoutMs, 10000)
              : 1000,
          signal: new AbortController().signal,
        }),
        (b) => port.close(b),
      );
      if (!closed.ok) return { ...result, cleanupError: closed.error };
      if (!closed.value.processStopped)
        return { ...result, cleanupError: unavailable() };
    } catch {
      return { ...result, cleanupError: unavailable() };
    }
    return result;
  }
  private static async initialize(
    port: ProviderAgentPort,
    configuration: ProviderConfiguration,
    budget: Budget,
    previous?: Session,
    fork?: ForkAdmission,
  ): Promise<Result<VerifiedProviderSession>> {
    if (budget.signal.aborted) return denied();
    const workspaceId = workspaceIdentity(configuration.workingDirectory);
    if (
      previous &&
      (previous.binding.workspaceId !== workspaceId ||
        !same(previous.namespace, configuration.namespace))
    )
      return denied();
    const controlled = configuration.permissions === "host_mediated";
    if (
      controlled
        ? typeof configuration.tools?.propose !== "function" ||
          typeof configuration.verifier?.verify !== "function"
        : configuration.permissions !== "tools_disabled" ||
          configuration.tools !== undefined ||
          configuration.verifier !== undefined
    )
      return denied();
    // Snapshot caller-owned identity before any external await. Endpoint/verifier identities
    // are captured separately, so concurrent caller mutation cannot switch the verifier.
    const namespace = structuredClone(configuration.namespace);
    const config = structuredClone(configuration.config),
      accountRef = configuration.accountRef,
      provider = configuration.provider;
    const tools = configuration.tools,
      verifier = configuration.verifier;
    if (previous && !port.resume) return denied();
    const settings = {
      ...configuration,
      namespace: structuredClone(namespace),
      config: structuredClone(config),
      accountRef,
    };
    let initialized;
    if (fork) {
      if (fork.rejection) return { ok: false, error: fork.rejection };
      if (!fork.port) return denied();
      fork.attempted = true;
      const result = (fork.result = await fork.port.forkSession(
        structuredClone(fork.request),
        settings,
        budget,
      ));
      if (result.certainty !== "created")
        return { ok: false, error: result.error };
      if (!same(result.source, fork.request)) return denied();
      const parent = fork.request.binding,
        child = result.value.binding;
      if (
        child.generation === parent.generation ||
        child.nativeSessionId === parent.nativeSessionId ||
        (parent.nativeThreadId !== undefined &&
          child.nativeThreadId === parent.nativeThreadId) ||
        child.providerVersion !== parent.providerVersion ||
        child.adapterVersion !== parent.adapterVersion
      )
        return denied();
      initialized = { ok: true as const, value: result.value };
    } else
      initialized = previous
        ? await port.resume!(
            structuredClone(previous.binding),
            settings,
            budget,
          )
        : await port.createSession(settings, budget);
    if (!initialized.ok) return initialized;
    const { binding, capabilities } = structuredClone(initialized.value);
    try {
      const limits = {
        maxBytes: 65536,
        maxTextBytes: 32768,
        maxDepth: 16,
        maxNodes: 2048,
      };
      decode(
        boundedJson(
          {
            schemaVersion: 2,
            kind: "session",
            namespace,
            revision: 0,
            lastSequence: 0,
            status: "active",
            binding,
            capabilities,
          },
          limits,
        ),
        limits,
      );
    } catch {
      return denied();
    }
    if (
      binding.workspaceId !== workspaceId ||
      binding.provider !== provider ||
      !same(binding.config, config) ||
      binding.accountRef !== accountRef ||
      capabilities.tools !== (controlled ? "host_mediated" : "disabled")
    )
      return denied();
    if (previous) {
      const prior = previous.binding;
      if (
        binding.workspaceId !== prior.workspaceId ||
        binding.generation === prior.generation ||
        binding.provider !== prior.provider ||
        binding.providerVersion !== prior.providerVersion ||
        binding.adapterVersion !== prior.adapterVersion ||
        binding.accountRef !== prior.accountRef ||
        !same(binding.config, prior.config) ||
        binding.nativeSessionId !== prior.nativeSessionId ||
        binding.nativeThreadId !== prior.nativeThreadId ||
        capabilities.continuation !== "across_processes"
      )
        return denied();
    }
    if (controlled) {
      const checked = await verifier!.verify(
        {
          binding: structuredClone(binding),
          capabilities: structuredClone(capabilities),
        },
        tools!,
        budget,
      );
      if (!checked.ok) return checked;
      if (!checked.value.platform || !checked.value.verificationRef)
        return denied();
    }
    if (budget.signal.aborted) return denied();
    return {
      ok: true,
      value: new VerifiedProviderSession(
        authority,
        port,
        namespace,
        binding,
        capabilities,
        tools,
        previous && {
          namespace: previous.namespace,
          binding: previous.binding,
        },
      ),
    };
  }
}
