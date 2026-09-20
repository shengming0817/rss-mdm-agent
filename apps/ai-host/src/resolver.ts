import type { HostOptions } from "@rss-mdm-agent/ai-host";
import {
  configurationFingerprint,
  type LocalConfiguration,
} from "./configuration.js";

/** One trusted resolver for both the local app and Store-bound recovery tests. */
export function localResolver(
  local: LocalConfiguration,
  path: string,
): HostOptions["resolve"] {
  const artifact = new URL("./provider.js", import.meta.url);
  artifact.searchParams.set("configuration", path);
  artifact.searchParams.set("fingerprint", configurationFingerprint(local));
  return async (caller, options, namespace) => {
    if (
      caller.tenantId !== local.caller.tenantId ||
      caller.principalId !== local.caller.principalId ||
      caller.authorityId !== local.caller.authorityId ||
      options.provider !== local.session.provider ||
      options.config.id !== local.session.config.id ||
      options.config.revision !== local.session.config.revision ||
      options.accountRef !== local.session.accountRef ||
      options.profile !== local.session.profile
    )
      throw new Error("local scope mismatch");
    return {
      configuration: {
        namespace,
        provider: options.provider,
        config: options.config,
        accountRef: options.accountRef,
        workingDirectory: local.workingDirectory,
        permissions:
          options.profile === "controlled_tools"
            ? "host_mediated"
            : "tools_disabled",
      },
      artifact: artifact.href,
      ...(options.profile === "controlled_tools"
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
  };
}
