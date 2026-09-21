import type { ProviderActivation } from "./connection.js";
import type { ConnectionSecrets } from "./secrets.js";
import { createHash } from "node:crypto";
import { rm } from "node:fs/promises";
import { join } from "node:path";
import type { HostOptions } from "@rss-mdm-agent/ai-host";
import { HostFailure } from "@rss-mdm-agent/ai-host";
import type { SessionStore } from "@rss-mdm-agent/ai-contract";
import type { LocalConfiguration } from "./configuration.js";
import { ConfigurationError } from "./configuration.js";
/** Exact historical connection revision is frozen before starting a worker. */
export function localResolver(
  local: LocalConfiguration,
  store: SessionStore,
  secrets: ConnectionSecrets,
): HostOptions["resolve"] {
  return async (
    caller,
    options,
    namespace,
    _budget,
    _previous,
    candidate,
    secret,
  ) => {
    try {
      if (
        caller.tenantId !== namespace.tenantId ||
        caller.principalId !== namespace.principalId ||
        caller.authorityId !== namespace.authorityId
      )
        throw new HostFailure({ code: "permission_denied", retry: "never" });
      const resolved = candidate
        ? { ok: true as const, value: candidate }
        : await store.connection(
            caller,
            options.config.id,
            Number(options.config.revision),
          );
      if (!resolved.ok) throw new HostFailure(resolved.error);
      const connection = resolved.value;
      if (
        options.config.revision !== String(connection.configRevision) ||
        connection.provider !== options.provider ||
        connection.profile !== options.profile ||
        connection.status === "deleted"
      )
        throw new HostFailure({ code: "invalid_input", retry: "never" });
      if (!candidate) {
        const current = await store.connection(caller, connection.connectionId);
        if (!current.ok || current.value.status === "deleted")
          throw new HostFailure({
            code: "connection_required",
            retry: "never",
          });
      }
      const activation: ProviderActivation = {
        local: {
          nativeDirectory: local.nativeDirectory,
          workingDirectory: local.workingDirectory,
        },
        connection,
        namespace,
        ...(candidate ? { verification: true } : {}),
        ...(connection.source.type === "custom_api"
          ? {
              secret: await secrets.read(
                caller,
                connection,
                secret,
                candidate
                  ? connection.configRevision - 1
                  : connection.configRevision,
              ),
            }
          : {}),
      };
      const artifact = new URL("./provider.js", import.meta.url);
      return {
        dispose: async () => {
          if (candidate) {
            const owner = createHash("sha256")
              .update(JSON.stringify(namespace))
              .digest("hex");
            await rm(join(local.nativeDirectory, "contexts", owner), {
              recursive: true,
              force: true,
            });
          }
        },
        configuration: {
          namespace,
          provider: connection.provider,
          config: options.config,

          workingDirectory: local.workingDirectory,
          permissions:
            connection.profile === "controlled_tools"
              ? "host_mediated"
              : "tools_disabled",
        },
        artifact: artifact.href,
        activation,
        ...(connection.profile === "controlled_tools"
          ? {
              admission: {
                verifier: {
                  verify: async (
                    session: import("@rss-mdm-agent/ai-contract").ProviderSessionBinding,
                  ) =>
                    process.platform === "darwin" &&
                    process.arch === "arm64" &&
                    session.binding.provider === "codex" &&
                    session.binding.providerVersion === "0.155.0" &&
                    session.capabilities.tools === "host_mediated"
                      ? {
                          ok: true as const,
                          value: {
                            platform: "darwin-arm64",
                            verificationRef: "codex-0.155.0-controlled",
                          },
                        }
                      : {
                          ok: false as const,
                          error: {
                            code: "unsupported_capability" as const,
                            retry: "never" as const,
                          },
                        },
                },
              },
            }
          : {}),
      };
    } catch (error) {
      if (error instanceof HostFailure) throw error;
      if (error instanceof ConfigurationError)
        throw new HostFailure({
          code:
            error.code === "authentication_required"
              ? "authentication_required"
              : error.code === "unsupported_capability"
                ? "unsupported_capability"
                : "invalid_input",
          retry: "never",
        });
      throw error;
    }
  };
}
