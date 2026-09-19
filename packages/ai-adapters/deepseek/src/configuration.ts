import { createHash } from "node:crypto";
import canonicalize from "canonicalize";
import { readFileSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { workspaceIdentity } from "@rss-mdm-agent/ai-contract/session";
import { isId } from "@rss-mdm-agent/ai-contract";
import type {
  Binding,
  Budget,
  Clock,
  ProviderConfiguration,
} from "@rss-mdm-agent/ai-contract";
import { same } from "./support.js";

export type DeepSeekConfiguration = ProviderConfiguration & {
  readonly provider: "deepseek";
};
export type DeepSeekDiagnostic = {
  stage:
    | "configuration"
    | "initialize"
    | "prompt"
    | "inspect"
    | "cancel"
    | "answer"
    | "tool_result"
    | "close"
    | "process"
    | "profile";
  reason:
    | "configuration_rejected"
    | "dependency_drift"
    | "restoration_failed"
    | "native_failure"
    | "protocol_failure"
    | "spawn_failed"
    | "process_exit"
    | "profile_drift"
    | "cleanup_failed"
    | "invalid_input"
    | "interaction_unavailable"
    | "budget_exhausted";
  generation?: string;
};
export interface ResolvedDeepSeekConfiguration {
  configuration: DeepSeekConfiguration;
  /** Trusted, private durable storage root; never the model's working directory. */
  persistenceDirectory: string;
  apiKey: string;
  model: string;
}
export interface DeepSeekAdapterOptions {
  resolveConfiguration(
    identity: Pick<Binding, "config" | "accountRef">,
    budget: Budget,
  ): Promise<ResolvedDeepSeekConfiguration>;
  clock?: Clock;
  callbackTimeoutMs?: number;
  /** Closed classifications only; never receives prompts, paths, credentials or native errors. */
  onDiagnostic?(diagnostic: DeepSeekDiagnostic): void;
}
export const manifest = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
export const HARNESS_VERSION: string =
  manifest.dependencies["@deepseek-ai/dsh-api-session-controller"];
export const ADAPTER_VERSION: string = manifest.version;
export const API_URL = "https://api.deepseek.com";
export const digest = (value: unknown): string =>
  createHash("sha256")
    .update(canonicalize(value) ?? "null")
    .digest("hex");
export function identity(c: ProviderConfiguration) {
  return {
    namespace: c.namespace,
    provider: c.provider,
    config: c.config,
    accountRef: c.accountRef,
    workspaceId: workspaceIdentity(c.workingDirectory),
    permissions: c.permissions,
  };
}
export function validateConfiguration(
  c: ProviderConfiguration,
  resolved: ResolvedDeepSeekConfiguration,
): asserts c is DeepSeekConfiguration {
  if (
    c.provider !== "deepseek" ||
    !c.namespace ||
    ![
      c.namespace.tenantId,
      c.namespace.principalId,
      c.namespace.authorityId,
      c.namespace.sessionId,
    ].every(isId) ||
    !isAbsolute(c.workingDirectory) ||
    !isAbsolute(resolved.persistenceDirectory) ||
    !resolved.apiKey ||
    !resolved.model ||
    !same(identity(c), identity(resolved.configuration)) ||
    c.tools !== resolved.configuration.tools ||
    c.verifier !== resolved.configuration.verifier ||
    !["tools_disabled", "host_mediated"].includes(c.permissions)
  )
    throw Error("invalid configuration");
}
export function sessionPrefix(
  c: ProviderConfiguration,
  r: ResolvedDeepSeekConfiguration,
  composition: string,
): string {
  return `rss_${digest([identity(c), resolve(r.persistenceDirectory), r.model, composition])}_`;
}
