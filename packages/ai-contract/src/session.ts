import canonicalize from "canonicalize";
import { boundedJson, decode } from "./codec.js";
import type { Binding, Capabilities } from "./wire.js";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  Result,
  ToolEndpoint,
} from "./ports.js";

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
  readonly #previous?: Binding;
  private constructor(
    token: symbol,
    binding: Binding,
    capabilities: Capabilities,
    tools?: ToolEndpoint,
    previous?: Binding,
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
  restores(previous: Binding): boolean {
    return this.#previous !== undefined && same(previous, this.#previous);
  }
  static async restore(
    port: ProviderAgentPort,
    previous: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<VerifiedProviderSession>> {
    const prior = structuredClone(previous);
    if (!port.resume || budget.signal.aborted) return denied();
    const resume = port.resume;
    const adapter = {
      createSession: (config: ProviderConfiguration, b: Budget) =>
        resume.call(port, structuredClone(prior), config, b),
    };
    const result = await VerifiedProviderSession.open(
      adapter,
      configuration,
      budget,
    );
    if (!result.ok) return result;
    const next = result.value.binding;
    if (
      next.generation === prior.generation ||
      next.provider !== prior.provider ||
      next.providerVersion !== prior.providerVersion ||
      next.adapterVersion !== prior.adapterVersion ||
      next.accountRef !== prior.accountRef ||
      !same(next.config, prior.config) ||
      next.nativeSessionId !== prior.nativeSessionId ||
      result.value.capabilities.continuation !== "across_processes"
    )
      return denied();
    return {
      ok: true,
      value: new VerifiedProviderSession(
        authority,
        next,
        result.value.capabilities,
        result.value.#tools,
        prior,
      ),
    };
  }
  static async open(
    port: Pick<ProviderAgentPort, "createSession">,
    configuration: ProviderConfiguration,
    budget: Budget,
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
    const initialized = await port.createSession(
      { ...configuration, config: structuredClone(config), accountRef },
      budget,
    );
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
      ),
    };
  }
}
