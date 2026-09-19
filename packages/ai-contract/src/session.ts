import canonicalize from "canonicalize";
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
  private constructor(
    token: symbol,
    binding: Binding,
    capabilities: Capabilities,
    tools?: ToolEndpoint,
  ) {
    if (token !== authority) throw new TypeError("unverified provider session");
    this.#binding = structuredClone(binding);
    this.#capabilities = structuredClone(capabilities);
    this.#tools = tools;
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
  static async open(
    port: ProviderAgentPort,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<VerifiedProviderSession>> {
    return this.admit(port, configuration, budget);
  }
  static async resume(
    port: ProviderAgentPort,
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<VerifiedProviderSession>> {
    if (!port.resume)
      return {
        ok: false,
        error: { code: "unsupported_capability", retry: "never" },
      };
    return this.admit(port, configuration, budget, binding);
  }
  private static async admit(
    port: ProviderAgentPort,
    configuration: ProviderConfiguration,
    budget: Budget,
    prior?: Binding,
  ): Promise<Result<VerifiedProviderSession>> {
    if (
      budget.signal.aborted ||
      !Number.isSafeInteger(budget.timeoutMs) ||
      budget.timeoutMs <= 0
    )
      return denied();
    const deadline = Date.now() + budget.timeoutMs;
    const remaining = (): Budget => ({
      ...budget,
      timeoutMs: deadline - Date.now(),
    });
    let opened = false,
      admitted = false;
    try {
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
      // Snapshot caller-owned identities before any external await. New and resumed
      // incarnations use the same verifier and endpoint admission funnel.
      const request = {
        ...configuration,
        config: structuredClone(configuration.config),
      };
      const tools = request.tools,
        verifier = request.verifier;
      const previous = prior ? structuredClone(prior) : undefined;
      if (
        previous &&
        (previous.provider !== request.provider ||
          !same(previous.config, request.config) ||
          previous.accountRef !== request.accountRef)
      )
        return denied();
      const initialized = previous
        ? await port.resume!(
            structuredClone(previous),
            { ...request, config: structuredClone(request.config) },
            remaining(),
          )
        : await port.createSession(
            { ...request, config: structuredClone(request.config) },
            remaining(),
          );
      if (!initialized.ok) return initialized;
      opened = true;
      const { binding, capabilities } = structuredClone(initialized.value);
      if (
        binding.provider !== request.provider ||
        !same(binding.config, request.config) ||
        binding.accountRef !== request.accountRef ||
        capabilities.tools !== (controlled ? "host_mediated" : "disabled") ||
        (previous &&
          (capabilities.continuation !== "across_processes" ||
            binding.nativeSessionId !== previous.nativeSessionId ||
            binding.generation === previous.generation))
      )
        return denied();
      if (budget.signal.aborted || remaining().timeoutMs <= 0) return denied();
      if (controlled) {
        const checked = await verifier!.verify(
          {
            binding: structuredClone(binding),
            capabilities: structuredClone(capabilities),
          },
          tools!,
          remaining(),
        );
        if (!checked.ok) return checked;
        if (!checked.value.platform || !checked.value.verificationRef)
          return denied();
      }
      if (budget.signal.aborted || remaining().timeoutMs <= 0) return denied();
      const value = new VerifiedProviderSession(
        authority,
        binding,
        capabilities,
        tools,
      );
      admitted = true;
      return { ok: true, value };
    } catch {
      return denied();
    } finally {
      if (opened && !admitted) {
        // Admission failure never exposes a live unverified session. Cleanup gets
        // its own bounded budget even if the caller's operation was aborted.
        const timeoutMs = Math.min(budget.timeoutMs, 15000);
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
          await Promise.race([
            port.close({ timeoutMs, signal: AbortSignal.timeout(timeoutMs) }),
            new Promise<void>((resolve) => {
              timer = setTimeout(resolve, timeoutMs);
            }),
          ]);
        } catch {
          /* The caller still owns the port and may retry close. */
        } finally {
          clearTimeout(timer);
        }
      }
    }
  }
}
