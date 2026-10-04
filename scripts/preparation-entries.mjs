import { readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const npm = (name) => `node:@rss-mdm-agent/${name}`;
const host = [npm("ai-host-app"), npm("ai-client"), "worker"];
const types = (suite) => [
  "pnpm",
  [
    "exec",
    "tsc",
    "--noEmit",
    "--strict",
    "--skipLibCheck",
    "--module",
    "NodeNext",
    "--target",
    "ES2022",
    `tests/${suite}/protocol.types.ts`,
  ],
];
const tests = (suites) => [
  "node",
  [
    "--test",
    ...suites.flatMap((suite) =>
      readdirSync(join(root, "tests", suite))
        .filter((name) => name.endsWith(".test.mjs"))
        .sort()
        .map((name) => `tests/${suite}/${name}`),
    ),
  ],
];
const script = (name) => ["node", [`scripts/${name}.mjs`]];
export function entry(name, args = []) {
  if (name.startsWith("package:"))
    return { requirements: [npm(name.slice(8))], commands: [] };
  if (name === "build:workspace")
    return {
      requirements: args.length
        ? args.map((name) => `node:${name}`)
        : ["node:*"],
      commands: [],
    };
  const specs = {
    build: { requirements: [npm("desktop")], commands: [] },
    "build:ai-contract": { requirements: [npm("ai-contract")], commands: [] },
    "build:ai-access": {
      requirements: [
        "ai-access",
        "ai-client",
        "ai-ui-bridge",
        "execution-bindings",
      ].map(npm),
      commands: [],
    },
    "build:ai-store": { requirements: [npm("ai-store-sqlite")], commands: [] },
    "build:ai-host": { requirements: host, commands: [] },
    "check:boundaries": {
      requirements: ["boundaries"],
      commands: [["node", ["--test", "scripts/boundaries.test.mjs"]]],
    },
    "test:ai-store": {
      requirements: [npm("ai-store-sqlite"), "privateWorker"],
      commands: [
        ["node", ["scripts/run-private-tests.mjs", "tests/ai-store-recovery"]],
      ],
    },
    "test:ai-host": {
      requirements: [...host, "execution", "hostFixture"],
      commands: [["node", ["scripts/run-private-tests.mjs", "tests/ai-host"]]],
    },
    "bundle:ai-host": {
      requirements: [...host, "execution"],
      commands: [script("bundle-ai-host")],
    },
    "test:ai-acceptance": {
      requirements: [...host, "execution"],
      commands: [script("check-ai-acceptance")],
    },
    "check:assistant": {
      requirements: [npm("desktop"), ...host],
      commands: [script("check-assistant-browser")],
    },
    "dev:assistant-fixture": {
      requirements: [npm("desktop"), npm("ai-host-app"), "worker"],
      commands: [["node", ["tests/assistant/server.mjs", "--serve"]]],
    },
    "check:connection-sources": {
      requirements: host,
      commands: [script("check-connection-sources")],
    },
    "check:ai-contract": { requirements: ["check:ai"], commands: [] },
    "check:codex-protocol": { requirements: ["check:codex"], commands: [] },
  };
  for (const [suffix, pkg, suite] of [
    ["contract", "ai-contract", "ai-contract"],
    ["access", "ai-access", "ai-access"],
    ["claude", "ai-adapter-claude", "ai-adapters/claude"],
    ["codex", "ai-adapter-codex", "ai-adapters/codex"],
    ["deepseek", "ai-adapter-deepseek", "ai-adapters/deepseek"],
  ]) {
    const requirements =
      suffix === "access" ? specs["build:ai-access"].requirements : [npm(pkg)];
    specs[`build:ai-${suffix}`] ??= { requirements, commands: [] };
    if (name === `test:ai-${suffix}`)
      return { requirements, commands: [types(suite), tests([suite])] };
  }
  if (!specs[name]) throw Error(`unknown prepared entry: ${name}`);
  return specs[name];
}
