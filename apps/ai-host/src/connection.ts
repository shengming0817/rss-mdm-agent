import { createHash } from "node:crypto";
import { parse as parseToml } from "@iarna/toml";
import { constants } from "node:fs";
import { open, lstat, realpath, writeFile, mkdir } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import { readPrivateFile } from "./private-file.js";
import {
  ConfigurationError,
  endpoint,
  type LocalConfiguration,
} from "./configuration.js";
import { nativeCredential } from "./credentials.js";
import type { Connection, Namespace } from "@rss-mdm-agent/ai-contract";
import type { ResolvedCodexConfiguration } from "@rss-mdm-agent/ai-adapter-codex";
import type { ResolvedClaudeConfiguration } from "@rss-mdm-agent/ai-adapter-claude";
export interface ProviderSnapshot {
  local: LocalConfiguration;
  connection: Connection;
  namespace: Namespace;
  generation: string;
  verification?: true;
}
export interface ResolvedConnection {
  model?: string;
  apiUrl: string;
  credential: ResolvedClaudeConfiguration["credential"];
  codex?: ResolvedCodexConfiguration["authentication"];
}
/** User-managed files may be readable by others, but never writable by other users.
 * Same descriptor, no symlink following, bounded reads; no file values in errors. */
export async function readUserFile(
  path: string,
  secret = true,
): Promise<string> {
  const directory = await lstat(dirname(path));
  if (
    !isAbsolute(path) ||
    !directory.isDirectory() ||
    directory.isSymbolicLink() ||
    directory.mode & 0o022 ||
    (process.getuid && directory.uid !== process.getuid()) ||
    (await realpath(dirname(path))) !== dirname(path)
  )
    throw new ConfigurationError("configuration_invalid");
  const before = await lstat(path);
  if (
    !before.isFile() ||
    before.isSymbolicLink() ||
    before.mode & (secret ? 0o077 : 0o022) ||
    (process.getuid && before.uid !== process.getuid())
  )
    throw new ConfigurationError("configuration_invalid");
  const file = await open(
    path,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const stat = await file.stat();
    if (
      !stat.isFile() ||
      stat.ino !== before.ino ||
      stat.dev !== before.dev ||
      stat.size > 262144
    )
      throw new Error();
    const data = Buffer.alloc(262145);
    let used = 0;
    while (used < data.length) {
      const { bytesRead } = await file.read(
        data,
        used,
        data.length - used,
        null,
      );
      if (!bytesRead) return data.subarray(0, used).toString("utf8");
      used += bytesRead;
    }
    throw new Error();
  } finally {
    await file.close();
  }
}
const text = (value: unknown): string => {
  if (typeof value !== "string" || !value.trim() || value.length > 16384)
    throw new ConfigurationError("authentication_required");
  return value;
};
async function optional(path: string): Promise<string | undefined> {
  try {
    return await readUserFile(path, false);
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
}
export function principalPath(snapshot: ProviderSnapshot): string {
  const key = createHash("sha256")
    .update(
      JSON.stringify([
        snapshot.namespace.tenantId,
        snapshot.namespace.principalId,
        snapshot.namespace.authorityId,
        snapshot.connection.connectionId,
        snapshot.connection.configRevision,
      ]),
    )
    .digest("hex");
  return join(snapshot.local.nativeDirectory, "principals", key + ".json");
}
/** Non-secret observed account lineage. No token/key hashes act as identities. */
async function bindPrincipal(
  snapshot: ProviderSnapshot,
  principal: object,
): Promise<void> {
  const path = principalPath(snapshot),
    directory = dirname(path);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const content = JSON.stringify(principal);
  try {
    await writeFile(path, content, {
      flag: snapshot.verification ? "w" : "wx",
      mode: 0o600,
    });
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code !== "EEXIST") throw e;
  }
  if ((await readPrivateFile(path, 4096)) !== content)
    throw new ConfigurationError("authentication_required");
}
export async function resolveConnection(
  snapshot: ProviderSnapshot,
): Promise<ResolvedConnection> {
  const { local, connection, namespace } = snapshot,
    source = connection.source;
  if (source.type === "custom_api") {
    const { value } = await nativeCredential<{ value: string }>(
      local.credentialSocket,
      {
        type: "credential",
        userId: namespace.principalId,
        generation: snapshot.generation,
        credentialRef: connection.credentialRef,
      },
    );
    const apiUrl = endpoint(source.apiUrl),
      credential = {
        type: source.credentialType ?? "api_key",
        value: text(value),
      };
    return {
      model: source.model,
      apiUrl,
      credential,
      ...(connection.provider === "codex"
        ? {
            codex: {
              type: "api_key",
              apiUrl,
              apiKey: credential.value,
            } as const,
          }
        : {}),
    };
  }
  if (connection.provider === "codex") {
    const settings = parseToml(
      (await optional(join(source.directory, "config.toml"))) ?? "",
    ) as Record<string, any>;
    const profileName =
      source.type === "existing_api"
        ? (source.profile ?? settings.profile)
        : undefined;
    const profile = profileName ? settings.profiles?.[profileName] : undefined;
    if (profileName && !profile)
      throw new ConfigurationError("configuration_invalid");
    const selected = { ...settings, ...profile },
      model = source.model ?? selected.model;
    if (model !== undefined) text(model);
    if (source.type === "existing_login") {
      const storage = settings.cli_auth_credentials_store ?? "file";
      if (!["file", "keyring", "auto", "ephemeral"].includes(storage))
        throw new ConfigurationError("configuration_invalid");
      const read = () =>
        nativeCredential<{ accessToken: string; accountId: string }>(
          local.credentialSocket,
          {
            type: "codex",
            userId: namespace.principalId,
            generation: snapshot.generation,
            directory: source.directory,
            storage,
          },
        );
      let current = await read();
      text(current.accessToken);
      text(current.accountId);
      await bindPrincipal(snapshot, {
        type: "chatgpt",
        accountId: current.accountId,
        directory: await realpath(source.directory),
        storage,
      });
      return {
        model,
        apiUrl: "https://api.openai.com/v1",
        credential: { type: "auth_token", value: current.accessToken },
        codex: {
          type: "chatgpt_tokens",
          ...current,
          refresh: async () => {
            const next = await read();
            if (
              next.accountId !== current.accountId ||
              !next.accessToken ||
              next.accessToken === current.accessToken
            )
              throw new ConfigurationError("authentication_required");
            current = next;
            return next;
          },
        },
      };
    }
    const provider = selected.model_provider ?? "openai",
      providerSettings = selected.model_providers?.[provider] ?? {};
    const auth = JSON.parse(
      await readUserFile(join(source.directory, "auth.json")),
    );
    const apiUrl = endpoint(
      providerSettings.base_url ??
        (provider === "openai" ? "https://api.openai.com/v1" : undefined),
    );
    if (providerSettings.experimental_bearer_token)
      await readUserFile(join(source.directory, "config.toml"));
    const value = text(
      providerSettings.env_key
        ? process.env[text(providerSettings.env_key)]
        : (providerSettings.experimental_bearer_token ?? auth.OPENAI_API_KEY),
    );
    return {
      model,
      apiUrl,
      credential: { type: "api_key", value },
      codex: { type: "api_key", apiUrl, apiKey: value },
    };
  }
  if (connection.provider === "claude") {
    if (source.type === "existing_login") {
      // SDK 0.3.277 accountInfo exposes optional display metadata, no stable account UUID.
      // Do not resume a different Keychain principal using cached email or token hashes.
      throw new ConfigurationError("unsupported_capability");
    }
    const settings = JSON.parse(
        (await optional(join(source.directory, "settings.json"))) ?? "{}",
      ),
      env = settings.env ?? {};
    const apiUrl = endpoint(
        env.ANTHROPIC_BASE_URL ?? "https://api.anthropic.com",
      ),
      model = source.model ?? settings.model ?? env.ANTHROPIC_MODEL;
    for (const [name, type] of [
      ["ANTHROPIC_API_KEY", "api_key"],
      ["ANTHROPIC_AUTH_TOKEN", "auth_token"],
    ] as const) {
      if (env[name]) {
        await readUserFile(join(source.directory, "settings.json"));
        return { model, apiUrl, credential: { type, value: text(env[name]) } };
      }
    }
  }
  throw new ConfigurationError("authentication_required");
}
