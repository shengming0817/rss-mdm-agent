import { Context, FiberState, type Fiber } from "@deepseek-ai/cordis";
import Agents from "@deepseek-ai/dsh-agent";
import AgentLoop from "@deepseek-ai/dsh-agent-loop";
import AgentDefaultModel from "@deepseek-ai/dsh-agent-default-model";
import Sessions from "@deepseek-ai/dsh-session";
import Projections from "@deepseek-ai/dsh-session-projection";
import Persistence from "@deepseek-ai/dsh-session-persistence-jsonl";
import Query from "@deepseek-ai/dsh-session-query-sqlite";
import * as Checkpoint from "@deepseek-ai/dsh-session-checkpoint-policy";
import SystemPrompt from "@deepseek-ai/dsh-system-prompt";
import Tools from "@deepseek-ai/dsh-tools";
import Llm from "@deepseek-ai/dsh-llm";
import {
  DeepSeekAdapter,
  resolveAdapterOptions,
} from "@deepseek-ai/dsh-llm-deepseek";
import Gateway from "@deepseek-ai/dsh-api-gateway";
import Controller from "@deepseek-ai/dsh-api-session-controller";
import { TYPERT } from "@deepseek-ai/dsh-api-session-controller/typert";
import Typert from "@deepseek-ai/dsh-typert-registry";
import Attachments from "@deepseek-ai/dsh-attachment-local";
import FileUploads from "@deepseek-ai/dsh-client-file-upload";
import * as Connection from "@deepseek-ai/dsh-client-connection";
import Commands from "@deepseek-ai/dsh-commands";
import Credentials from "@deepseek-ai/dsh-credentials-local";
import Storage from "@deepseek-ai/dsh-storage";
import * as Json from "@deepseek-ai/dsh-storage-json";
import * as Domain from "@deepseek-ai/dsh-storage-domain";
import Workspace from "@deepseek-ai/dsh-workspace";
import Questions from "@deepseek-ai/dsh-user-questions";
import * as Ask from "@deepseek-ai/dsh-tool-ask-user";
import { join } from "node:path";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { digest, manifest } from "./configuration.js";
import { NativeFault } from "./protocol.js";

import type { Initialization } from "./protocol.js";
export type { Initialization } from "./protocol.js";
// Sole declaration of the installed profile. No Loader, discovery, settings, MCP,
// PTC, terminal, delegation or executable workspace instructions are mounted.
const assembly: readonly [string, any, (i: Initialization) => any][] = [
  ["dsh-typert-registry", Typert, () => undefined],
  ["dsh-llm", Llm, () => undefined],
  ["dsh-session", Sessions, () => undefined],
  ["dsh-session-projection", Projections, () => undefined],
  [
    "dsh-system-prompt",
    SystemPrompt,
    () => ({
      includeHarnessIdentity: false,
      includeRuntimeContext: false,
      personaPrefix:
        "You are a helpful assistant. Use only the tools supplied with this request.",
      personaSuffix: "",
    }),
  ],
  ["dsh-tools", Tools, () => ({ mode: "native" })],
  ["dsh-agent", Agents, () => undefined],
  [
    "dsh-session-persistence-jsonl",
    Persistence,
    (i) => ({
      root: join(i.persistenceDirectory, i.scope, "sessions"),
      compression: "none",
    }),
  ],
  [
    "dsh-session-query-sqlite",
    Query,
    () => ({ path: ":memory:", openAt: "never" }),
  ],
  ["dsh-storage", Storage, () => undefined],
  [
    "dsh-storage-json",
    Json,
    (i) => ({ root: join(i.persistenceDirectory, i.scope, "storage") }),
  ],
  ["dsh-storage-domain", Domain, () => ({ backend: "json" })],
  ["dsh-workspace", Workspace, () => undefined],
  [
    "dsh-agent-default-model",
    AgentDefaultModel,
    (i) => ({ provider: "deepseek-official", model: i.model }),
  ],
  [
    "dsh-attachment-local",
    Attachments,
    () => ({ dshHome: process.env.DSH_HOME }),
  ],
  [
    "dsh-credentials-local",
    Credentials,
    () => ({ dshHome: process.env.DSH_HOME, watch: false }),
  ],
  ["dsh-client-connection", Connection, () => ({ trustedHosts: [] })],
  ["dsh-commands", Commands, () => undefined],
  ["dsh-client-file-upload", FileUploads, () => undefined],
  ["dsh-agent-loop", AgentLoop, () => ({ agents: [] })],
  ["dsh-session-checkpoint-policy", Checkpoint, () => undefined],
  ["dsh-user-questions", Questions, () => undefined],
  ["dsh-tool-ask-user", Ask, () => undefined],
  ["dsh-api-gateway", Gateway, () => ({ websocketHeartbeatIntervalMs: 2000 })],
  ["dsh-api-session-controller", Controller, () => ({ nativeOpen: false })],
];
export const TOOL_PROFILE = {
  question: "ask_user_question",
  proposal: {
    name: "host_propose",
    description:
      "Propose a business operation to the host. This tool does not grant execution authority.",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string" },
        arguments: { type: "object", additionalProperties: true },
      },
      required: ["name", "arguments"],
      additionalProperties: false,
    },
  },
};
const modelConfiguration = (i: Initialization) => ({
  protocol: "chat-completions" as const,
  baseURL: i.apiUrl,
  apiKeyEnv: "DEEPSEEK_API_KEY",
  thinking: "disabled" as const,
  models: [{ id: i.model }],
  maxTokens: 4096,
});
export const COMPOSITION_ID = digest([
  assembly.map(([name, , config]) => [name, config.toString()]),
  manifest.dependencies,
  TOOL_PROFILE,
  modelConfiguration.toString(),
  "native;deny-guard-v1;question;host-propose;chat-completions;checkpoint",
]);
export const ACTIVE_PROFILE_ID = digest([
  COMPOSITION_ID,
  assembly.map(([name]) => [name, "active"]),
]);
export async function assemble(
  ctx: Context,
  i: Initialization,
  onDrift: () => void = () => {},
) {
  const require = createRequire(import.meta.url);
  for (const [name, version] of Object.entries(manifest.dependencies)) {
    if (!name.startsWith("@deepseek-ai/")) continue;
    const installed = JSON.parse(
      readFileSync(require.resolve(`${name}/package.json`), "utf8"),
    );
    if (installed.version !== version)
      throw new NativeFault("dependency_drift");
  }
  const fibers: { name: string; fiber: Fiber; config: string }[] = [];
  for (const [name, plugin, config] of assembly) {
    const fiber = await ctx.plugin(plugin, config(i));
    fibers.push({ name, fiber, config: digest(fiber.config ?? null) });
  }
  ctx.typert.register(TYPERT as Parameters<typeof ctx.typert.register>[0]);
  const connection = resolveAdapterOptions(modelConfiguration(i));
  ctx.llm.registerAdapter(
    ["deepseek-official"],
    new DeepSeekAdapter({
      options: () => connection,
      resolveApiKey: async () => i.apiKey,
      resolveUserId: () => "rss-adapter" as any,
      prepareExtensions: async () => ({ fields: {}, accept: async () => {} }),
    }),
  );
  let live = true;
  const drift = () => {
    if (live) {
      live = false;
      onDrift();
    }
  };
  const verify = () => {
    if (
      !live ||
      fibers.some(
        ({ fiber, config }) =>
          fiber.state !== FiberState.ACTIVE ||
          digest(fiber.config ?? null) !== config,
      )
    ) {
      drift();
      throw new NativeFault("profile_drift");
    }
  };
  verify();
  ctx.on("internal/status", (fiber) => {
    if (
      fibers.some((f) => f.fiber === fiber) &&
      fiber.state !== FiberState.ACTIVE
    )
      drift();
  });
  ctx.on(
    "internal/update",
    function () {
      if (fibers.some((f) => f.fiber === this)) drift();
      throw Error("configuration update disabled");
    },
    { global: true },
  );
  return {
    verify,
    activation: digest([
      COMPOSITION_ID,
      fibers.map(({ name }) => [name, "active"]),
    ]),
  };
}
