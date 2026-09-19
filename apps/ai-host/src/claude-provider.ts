import { readFile, lstat } from "node:fs/promises";
import { createClaudeAdapter } from "@rss-mdm-agent/ai-adapter-claude";
import type { WorkerFactory } from "@rss-mdm-agent/ai-host/worker";
import { readConfiguration } from "./configuration.js";
/** Loaded exclusively in the activated worker; credentials and SDK stay there. */
export const createProvider: WorkerFactory = async ({
  configuration,
  tools,
}) => {
  const path = new URL(import.meta.url).searchParams.get("configuration");
  if (!path) throw new Error("missing composition input");
  const local = await readConfiguration(path);
  if (
    local.session.accountRef !== configuration.accountRef ||
    local.session.config.id !== configuration.config.id ||
    local.session.config.revision !== configuration.config.revision ||
    local.workingDirectory !== configuration.workingDirectory
  )
    throw new Error("configuration identity changed");
  return createClaudeAdapter({
    tools,
    resolveConfiguration: async () => {
      const stat = await lstat(local.claude.credentialPath);
      if (
        !stat.isFile() ||
        stat.isSymbolicLink() ||
        (stat.mode & 0o077) !== 0 ||
        (process.getuid && stat.uid !== process.getuid()) ||
        stat.size > 16384
      )
        throw new Error("credential ownership");
      const credential = (
        await readFile(local.claude.credentialPath, "utf8")
      ).trim();
      return {
        configuration: { ...configuration, provider: "claude" },
        configurationDirectory: local.claude.configurationDirectory,
        apiUrl: local.claude.apiUrl,
        credential: { type: local.claude.credentialType, value: credential },
        ...(local.claude.model ? { model: local.claude.model } : {}),
      };
    },
  });
};
