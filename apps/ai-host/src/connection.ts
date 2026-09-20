import { createHash } from "node:crypto";
import { parse as parseToml } from "@iarna/toml";
import { parse as parseYaml } from "yaml";
import { constants } from "node:fs";
import { open, lstat, realpath, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import { readPrivateFile } from "./private-file.js";
import {
  ConfigurationError,
  configurationFingerprint,
  endpoint,
  type LocalConfiguration,
} from "./configuration.js";
import type { ResolvedCodexConfiguration } from "@rss-mdm-agent/ai-adapter-codex";
export interface Connection {
  model?: string;
  apiUrl: string;
  credential: import("@rss-mdm-agent/ai-adapter-claude").ResolvedClaudeConfiguration["credential"];
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
/** Credential reads occur only inside an activated provider worker. No user tool settings escape. */
export async function resolveConnection(
  local: LocalConfiguration,
): Promise<Connection> {
  const source = local.connection;
  if (source.source === "custom_endpoint") {
    const apiUrl = endpoint(source.apiUrl),
      value = text(
        (await readPrivateFile(source.credentialPath, 16384)).trim(),
      );
    return {
      model: source.model,
      apiUrl,
      credential: { type: source.credentialType, value },
      ...(local.session.provider === "codex"
        ? { codex: { type: "api_key" as const, apiUrl, apiKey: value } }
        : {}),
    };
  }
  if (local.session.provider === "codex") {
    const configPath = join(source.directory, "config.toml");
    const configText = (await optional(configPath)) ?? "";
    const settings = parseToml(configText) as Record<string, any>;
    const profileName = source.profile ?? settings.profile;
    const profile = profileName ? settings.profiles?.[profileName] : undefined;
    if (profileName && !profile)
      throw new ConfigurationError("configuration_invalid");
    const selected = { ...settings, ...profile };
    const authPath = join(source.directory, "auth.json");
    const auth = JSON.parse(await readUserFile(authPath));
    const provider = selected.model_provider ?? "openai";
    const providerSettings = selected.model_providers?.[provider] ?? {};
    const model = source.model ?? selected.model;
    if (model !== undefined && (typeof model !== "string" || !model.trim()))
      throw new ConfigurationError("configuration_invalid");
    const apiUrl = endpoint(
      providerSettings.base_url ??
        (provider === "openai" ? "https://api.openai.com/v1" : undefined),
    );
    if (
      auth.auth_mode === "chatgpt" &&
      (provider === "openai" || providerSettings.requires_openai_auth === true)
    ) {
      const tokens = () => ({
        accessToken: text(auth.tokens?.access_token),
        accountId: text(auth.tokens?.account_id),
      });
      const initial = tokens();
      return {
        model,
        apiUrl,
        credential: { type: "auth_token", value: initial.accessToken },
        codex: {
          type: "chatgpt_tokens",
          ...initial,
          ...(providerSettings.base_url ? { apiUrl } : {}),
          refresh: async () => {
            const updated = JSON.parse(await readUserFile(authPath));
            if (updated.auth_mode !== "chatgpt")
              throw new ConfigurationError("authentication_required");
            const next = {
              accessToken: text(updated.tokens?.access_token),
              accountId: text(updated.tokens?.account_id),
            };
            if (next.accountId !== initial.accountId)
              throw new ConfigurationError("authentication_required");
            return next;
          },
        },
      };
    }
    if (
      !providerSettings.env_key &&
      providerSettings.experimental_bearer_token &&
      (await readUserFile(configPath)) !== configText
    )
      throw new ConfigurationError("configuration_invalid");
    const key = text(
      providerSettings.env_key
        ? process.env[text(providerSettings.env_key)]
        : (providerSettings.experimental_bearer_token ?? auth.OPENAI_API_KEY),
    );
    return {
      model,
      apiUrl,
      credential: { type: "api_key", value: key },
      codex: { type: "api_key", apiUrl, apiKey: key },
    };
  }
  if (local.session.provider === "claude") {
    const settingsPath = join(source.directory, "settings.json");
    const settingsText = (await optional(settingsPath)) ?? "{}";
    const settings = JSON.parse(settingsText);
    const env = settings.env ?? {};
    const apiUrl = endpoint(
      env.ANTHROPIC_BASE_URL ??
        process.env.ANTHROPIC_BASE_URL ??
        "https://api.anthropic.com",
    );
    const model =
      source.model ??
      settings.model ??
      env.ANTHROPIC_MODEL ??
      process.env.ANTHROPIC_MODEL;
    if (model !== undefined) text(model);
    for (const [name, type] of [
      ["ANTHROPIC_API_KEY", "api_key"],
      ["ANTHROPIC_AUTH_TOKEN", "auth_token"],
      ["CLAUDE_CODE_OAUTH_TOKEN", "oauth_token"],
    ] as const) {
      const value = env[name] ?? process.env[name];
      if (value) {
        if (env[name] && (await readUserFile(settingsPath)) !== settingsText)
          throw new ConfigurationError("configuration_invalid");
        return { model, apiUrl, credential: { type, value: text(value) } };
      }
    }
    if (process.platform === "darwin")
      return {
        model,
        apiUrl,
        credential: { type: "user_login", sourceDirectory: source.directory },
      };
    const credentials = JSON.parse(
      await readUserFile(join(source.directory, ".credentials.json")),
    );
    return {
      model,
      apiUrl,
      credential: {
        type: "oauth_token",
        value: text(credentials.claudeAiOauth?.accessToken),
      },
    };
  }
  const credentials = parseYaml(
    await readUserFile(join(source.directory, ".credentials.yaml")),
    { maxAliasCount: 0 },
  );
  if (credentials?.version !== 1)
    throw new ConfigurationError("configuration_invalid");
  const settings =
    parseYaml(
      (await optional(join(source.directory, "settings.yaml"))) ?? "{}",
      { maxAliasCount: 0 },
    ) ?? {};
  const config: Record<string, any> = {};
  if (source.profile) {
    for (const name of ["cordis.yml", "cordis.patch.yml"]) {
      const rows = parseYaml(
        (await optional(
          join(source.directory, "profiles", source.profile, name),
        )) ?? "[]",
        { maxAliasCount: 0 },
      );
      if (!Array.isArray(rows))
        throw new ConfigurationError("configuration_invalid");
      for (const row of rows) {
        if (
          row?.name === "@deepseek-ai/dsh-llm-deepseek" ||
          row?.name === "@deepseek-ai/dsh-agent-default-model"
        )
          config[row.name] = { ...config[row.name], ...row.config };
      }
    }
  }
  const provider = config["@deepseek-ai/dsh-llm-deepseek"] ?? {};
  if (provider.protocol && provider.protocol !== "chat-completions")
    throw new ConfigurationError("configuration_invalid");
  return {
    model: text(
      source.model ??
        settings["agent-default-model"]?.model ??
        config["@deepseek-ai/dsh-agent-default-model"]?.model ??
        process.env.DEEPSEEK_MODEL,
    ),
    apiUrl: endpoint(
      provider.baseURL ??
        process.env.DEEPSEEK_BASE_URL ??
        "https://api.deepseek.com",
    ),
    credential: {
      type: "api_key",
      value: text(credentials.refs?.DEEPSEEK_API_KEY),
    },
  };
}

/** Private per-config lineage survives Host restart. Token refresh for one ChatGPT account
 * is allowed; endpoint/model/account/credential identity changes require a new revision. */
export async function bindConnection(
  local: LocalConfiguration,
  connection: Connection,
): Promise<void> {
  const hash = (value: unknown) =>
    createHash("sha256").update(JSON.stringify(value)).digest("hex");
  const key = hash([
    local.caller.tenantId,
    local.caller.principalId,
    local.caller.authorityId,
    local.session.provider,
    local.session.accountRef,
    local.session.config,
  ]);
  const identity = hash([
    configurationFingerprint(local),
    connection.apiUrl,
    connection.model ?? null,
    connection.codex?.type === "chatgpt_tokens"
      ? ["chatgpt", connection.codex.accountId]
      : connection.credential,
  ]);
  const path = join(dirname(local.databasePath), `connection-${key}.identity`);
  try {
    await writeFile(path, identity, { flag: "wx", mode: 0o600 });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
  }
  if ((await readPrivateFile(path, 128)) !== identity)
    throw new ConfigurationError("configuration_invalid");
}
