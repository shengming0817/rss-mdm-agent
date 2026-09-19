import canonicalize from "canonicalize";
import { withinBudget } from "./budget.js";
import { boundedJson, decode } from "./codec.js";
import type { Binding, Capabilities, Session, Failure } from "./wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  Result,
  ToolEndpoint,
} from "./ports.js";

export type AdmissionResult =
  | { ok: true; value: VerifiedProviderSession }
  | { ok: false; error: Failure; cleanupError?: Failure };
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

/** Nominal, process-local admission evidence. JSON and structural casts cannot mint it.
 * The injected verifier is trusted code owned by the composition root; its actual
 * platform containment proof remains adapter-owned. */
export class VerifiedProviderSession {
  readonly #binding: Binding;
  readonly #capabilities: Capabilities;
  readonly #tools?: ToolEndpoint;
  readonly #previous?: Pick<Session, "namespace" | "binding">;
  private constructor(
    token: symbol,
    binding: Binding,
    capabilities: Capabilities,
    tools?: ToolEndpoint,
    previous?: Pick<Session, "namespace" | "binding">,
  ) {
    if (token !== authority) throw new TypeError("unverified provider session");
    this.#binding = structuredClone(binding);
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
        config: structuredClone(configuration.config),
      };
      result = await withinBudget(
        () => budget,
        (b) => this.initialize(port, settings, b, prior),
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
  ): Promise<Result<VerifiedProviderSession>> {
    if (budget.signal.aborted) return denied();
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
    const config = structuredClone(configuration.config),
      accountRef = configuration.accountRef,
      provider = configuration.provider;
    const tools = configuration.tools,
      verifier = configuration.verifier;
    if (previous && !port.resume) return denied();
    const settings = {
      ...configuration,
      config: structuredClone(config),
      accountRef,
    };
    const initialized = previous
      ? await port.resume!(structuredClone(previous.binding), settings, budget)
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
            namespace: {
              tenantId: "verify",
              principalId: "verify",
              authorityId: "verify",
              sessionId: "verify",
            },
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
      binding.provider !== provider ||
      !same(binding.config, config) ||
      binding.accountRef !== accountRef ||
      capabilities.tools !== (controlled ? "host_mediated" : "disabled")
    )
      return denied();
    if (previous) {
      const prior = previous.binding;
      if (
        binding.generation === prior.generation ||
        binding.provider !== prior.provider ||
        binding.providerVersion !== prior.providerVersion ||
        binding.adapterVersion !== prior.adapterVersion ||
        binding.accountRef !== prior.accountRef ||
        !same(binding.config, prior.config) ||
        binding.nativeSessionId !== prior.nativeSessionId ||
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
