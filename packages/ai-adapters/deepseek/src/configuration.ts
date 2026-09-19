import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { workspaceIdentity } from "@rss-mdm-agent/ai-contract/session";
import type {
  Binding,
  Budget,
  Clock,
  ProviderConfiguration,
} from "@rss-mdm-agent/ai-contract";
import { same } from "./support.js";

export interface ResolvedDeepSeekConfiguration {
  configuration: ProviderConfiguration;
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
}
export const manifest = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
export const HARNESS_VERSION = "0.1.6-alpha.2";
export const ADAPTER_VERSION: string = manifest.version;
export const API_URL = "https://api.deepseek.com";
export const digest = (value: unknown): string =>
  createHash("sha256").update(JSON.stringify(value)).digest("hex");
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
): void {
  if (
    c.provider !== "deepseek" ||
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
