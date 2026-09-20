import { principalPath, type ProviderSnapshot } from "./connection.js";
import { nativeContext } from "./credentials.js";
import { createHash } from "node:crypto";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import type { HostOptions } from "@rss-mdm-agent/ai-host";
import type { SessionStore } from "@rss-mdm-agent/ai-contract";
import type { LocalConfiguration } from "./configuration.js";
import { readPrivateFile } from "./private-file.js";
/** Exact historical connection revision is frozen before starting a worker. */
export function localResolver(
  local: LocalConfiguration,
  store: SessionStore,
): HostOptions["resolve"] {
  return async (caller, options, namespace, _budget, _previous, candidate) => {
    if (
      caller.tenantId !== namespace.tenantId ||
      caller.principalId !== namespace.principalId ||
      caller.authorityId !== namespace.authorityId
    )
      throw new Error("scope mismatch");
    const resolved = candidate
      ? { ok: true as const, value: candidate }
      : await store.connection(
          caller,
          options.config.id,
          Number(options.config.revision),
        );
    if (!resolved.ok) throw new Error("connection_required");
    const connection = resolved.value;
    if (
      options.config.revision !== String(connection.configRevision) ||
      connection.provider !== options.provider ||
      connection.accountRef !== options.accountRef ||
      connection.profile !== options.profile ||
      connection.status === "deleted"
    )
      throw new Error("configuration identity changed");
    const content = JSON.stringify({
        local,
        connection,
        namespace,
        generation: (await nativeContext(local.usersPath, caller)).generation,
        ...(candidate ? { verification: true } : {}),
      }),
      fingerprint = createHash("sha256").update(content).digest("hex");
    const directory = join(local.nativeDirectory, "snapshots");
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const path = join(directory, fingerprint + ".json");
    try {
      await writeFile(path, content, { flag: "wx", mode: 0o600 });
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "EEXIST") throw e;
    }
    if ((await readPrivateFile(path, 65536)) !== content)
      throw new Error("configuration identity changed");
    const artifact = new URL("./provider.js", import.meta.url);
    artifact.searchParams.set("snapshot", path);
    artifact.searchParams.set("fingerprint", fingerprint);
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
        await rm(path, { force: true });
        if (candidate) {
          const saved = await store.connection(
            caller,
            candidate.connectionId,
            candidate.configRevision,
          );
          if (!saved.ok && saved.error.code === "connection_required")
            await rm(principalPath(JSON.parse(content) as ProviderSnapshot), {
              force: true,
            });
        }
      },
      configuration: {
        namespace,
        provider: connection.provider,
        config: options.config,
        accountRef: connection.accountRef,
        workingDirectory: local.workingDirectory,
        permissions:
          connection.profile === "controlled_tools"
            ? "host_mediated"
            : "tools_disabled",
      },
      artifact: artifact.href,
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
  };
}
