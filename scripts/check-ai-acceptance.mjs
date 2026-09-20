import { run } from "node:test";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, mkdirSync, writeFileSync } from "node:fs";
import { isDeepStrictEqual } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";
import { sourceState, sameCommittedSource } from "./source-state.mjs";

const engines = ["codex", "claude", "deepseek"];
export const required = engines.flatMap((provider) =>
  [
    "native-basic-ledger-queue-replay",
    "native-cancel-request-terminal",
    "native-steer-difference",
    "production-controlled-admission",
    "provider-received-host-fact-lost",
    "host-restart-display-native-context",
    ...(provider === "codex" ? [] : ["question-answer-race-lost-callback"]),
    ...(provider === "deepseek" ? ["cancel-without-terminal-recovery"] : []),
  ].map((scenario) => `${provider}:${scenario}`),
);

export function capabilities(provider, tools = "disabled") {
  return {
    continuation: "across_processes",
    cancellation: "request_only",
    tools,
    steer: provider === "codex" ? "supported" : "unsupported",
    fork: provider === "codex" ? "supported" : "unsupported",
    subagent: "unsupported",
    terminal: "unsupported",
    structuredQuestion: provider === "codex" ? "unsupported" : "supported",
    multimodal: "unsupported",
  };
}
/** Bind every row to the fixed adapter profile implementation, including plugin wiring. */
export function profileDigest(provider) {
  if (!engines.includes(provider)) throw Error("unknown provider");
  const directory = new URL(
    `../packages/ai-adapters/${provider}/src/`,
    import.meta.url,
  );
  const hash = createHash("sha256");
  for (const file of readdirSync(directory, { recursive: true })
    .filter((f) => f.endsWith(".ts"))
    .sort())
    hash.update(file + "\0").update(readFileSync(new URL(file, directory)));
  return hash.digest("hex");
}
const id = (value) =>
  typeof value === "string" && value.length > 0 && value.length <= 512;
/** Observed request definitions only; never derive this inventory from capability declarations. */
export function nativeToolInventory(requests) {
  if (!Array.isArray(requests) || requests.length === 0)
    throw new Error("No native model request");
  const names = requests.flatMap((request) => {
    const tools = request.tools ?? [];
    if (!Array.isArray(tools)) throw new Error("Invalid native tool inventory");
    return tools.flatMap((tool) => {
      if (tool?.type === "namespace") {
        if (!id(tool.name) || !Array.isArray(tool.tools))
          throw new Error("Invalid native tool inventory");
        return tool.tools.map((child) => {
          if (!id(child?.name))
            throw new Error("Invalid native tool inventory");
          return `${tool.name}__${child.name}`;
        });
      }
      const name = tool?.name ?? tool?.function?.name;
      if (!id(name)) throw new Error("Invalid native tool inventory");
      return name;
    });
  });
  return [...new Set(names)].sort();
}
function validRow(row, installations) {
  const controlled = row.scenario === "production-controlled-admission";
  const rejected = controlled && row.provider !== "codex";
  const expected = installations?.[row.provider],
    binding = row.binding;
  if (
    !expected ||
    row.a06 !== 1 ||
    row.profile !== (controlled ? "controlled_tools" : "conversation") ||
    row.profileSourceSha256 !== expected.profileSourceSha256 ||
    !/^[a-f0-9]{64}$/.test(row.profileSourceSha256 ?? "") ||
    row.proof !==
      (rejected
        ? "production_admission"
        : controlled
          ? "real_process_local_model_rust_s1"
          : "real_process_local_model") ||
    row.result !==
      (rejected ||
      (row.scenario === "native-steer-difference" && row.provider !== "codex")
        ? "unsupported"
        : "supported")
  )
    return false;
  if (rejected)
    return (
      row.modelRequests === 0 &&
      binding === undefined &&
      row.capabilities === undefined &&
      row.nativeTools === undefined
    );
  return (
    binding?.provider === row.provider &&
    binding.providerVersion === expected.providerVersion &&
    binding.adapterVersion === expected.adapterVersion &&
    binding.config?.id === "local" &&
    binding.config.revision === (controlled ? "2" : "1") &&
    binding.accountRef === "test-account" &&
    id(binding.generation) &&
    id(binding.nativeSessionId) &&
    (row.provider !== "codex" || id(binding.nativeThreadId)) &&
    ["nativeRunId", "nativeRequestId"].every(
      (key) => binding[key] === undefined || id(binding[key]),
    ) &&
    Number.isSafeInteger(row.modelRequests) &&
    row.modelRequests > 0 &&
    isDeepStrictEqual(
      row.nativeTools,
      row.provider === "codex"
        ? controlled
          ? [
              "list_mcp_resource_templates",
              "list_mcp_resources",
              "mcp__rss_host__propose",
              "read_mcp_resource",
            ]
          : []
        : [row.provider === "claude" ? "AskUserQuestion" : "ask_user_question"],
    ) &&
    isDeepStrictEqual(
      row.capabilities,
      capabilities(row.provider, controlled ? "host_mediated" : "disabled"),
    )
  );
}

export function assess(rows, tests, context) {
  const keys = rows.map((row) => `${row.provider}:${row.scenario}`);
  const complete =
    keys.length === required.length &&
    new Set(keys).size === required.length &&
    required.every((key) => keys.includes(key));
  return {
    complete,
    passed:
      complete &&
      context?.sourceVerified === true &&
      context.runtime?.platform === "darwin" &&
      context.runtime.arch === "arm64" &&
      context.runtime.node ===
        "v" +
          JSON.parse(readFileSync(new URL("../package.json", import.meta.url)))
            .engines.node &&
      tests.length === required.length &&
      tests.every((t) => t.status === "pass") &&
      rows.every((r) => validRow(r, context.installations)) &&
      new Set(rows.filter((r) => r.binding).map((r) => r.binding.generation))
        .size === rows.filter((r) => r.binding).length,
    missing: required.filter((key) => !keys.includes(key)),
  };
}

async function main() {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const start = sourceState(root);
  const hash = (name) =>
    createHash("sha256")
      .update(readFileSync(new URL("../" + name, import.meta.url)))
      .digest("hex");
  const locks = () => ({
    pnpm: hash("pnpm-lock.yaml"),
    cargo: hash("Cargo.lock"),
  });
  const before = locks(),
    rows = [],
    tests = [];
  const runtime = {
    platform: process.platform,
    arch: process.arch,
    node: process.version,
  };
  const [codex, claude, deepseek, assembly] = await Promise.all([
    import("../packages/ai-adapters/codex/dist/runtime.js"),
    import("../packages/ai-adapters/claude/dist/configuration.js"),
    import("../packages/ai-adapters/deepseek/dist/configuration.js"),
    import("../packages/ai-adapters/deepseek/dist/assembly.js"),
  ]);
  const versions = {
    codex: codex.CODEX_VERSION,
    claude: claude.PROVIDER_VERSION,
    deepseek: `harness-${deepseek.HARNESS_VERSION}.${assembly.COMPOSITION_ID}`,
  };
  const installations = Object.fromEntries(
    engines.map((provider) => [
      provider,
      {
        providerVersion: versions[provider],
        adapterVersion: JSON.parse(
          readFileSync(
            new URL(
              `../packages/ai-adapters/${provider}/package.json`,
              import.meta.url,
            ),
          ),
        ).version,
        profileSourceSha256: profileDigest(provider),
      },
    ]),
  );
  const files = ["ai-provider-conformance", "ai-recovery-integration"].flatMap(
    (suite) => {
      const directory = new URL("../tests/" + suite + "/", import.meta.url);
      return readdirSync(directory)
        .filter((name) => name.endsWith(".test.mjs"))
        .sort()
        .map((name) => fileURLToPath(new URL(name, directory)));
    },
  );
  for await (const event of run({ files, concurrency: 1, timeout: 120000 })) {
    if (
      event.type === "test:diagnostic" &&
      event.data.message.startsWith('{"a06":1,')
    ) {
      rows.push(JSON.parse(event.data.message));
    } else if (["test:pass", "test:fail"].includes(event.type)) {
      const status =
        event.type === "test:pass" && !event.data.skip && !event.data.todo
          ? "pass"
          : "fail";
      tests.push({ name: event.data.name, status });
      console.log(`[a06] ${status.toUpperCase()} ${event.data.name}`);
      if (status === "fail") console.error(event.data.details?.error);
    } else if (event.type === "test:stderr")
      process.stderr.write(event.data.message);
  }
  const end = sourceState(root),
    after = locks();
  const sourceVerified =
    sameCommittedSource(start, end) &&
    JSON.stringify(before) === JSON.stringify(after);
  const verdict = assess(rows, tests, {
    sourceVerified,
    runtime,
    installations,
  });
  const receipt = {
    schemaVersion: 1,
    command: "pnpm test:ai-acceptance",
    timestamp: new Date().toISOString(),
    source: { start, end },
    lockSha256: { before, after },
    sourceVerified,
    runtime,
    installations,
    configuration: {
      modelEndpoint: "loopback_protocol_fixture",
      credentials: "fixture_only",
      nativePlugins: "fixed adapter profiles from the recorded source and lock",
    },
    verdict,
    tests,
    rows,
    notVerified: [
      "external real models (not run by this command)",
      "Windows",
      "OS T3",
      "same-UID arbitrary filesystem/network/IPC containment",
      "enterprise identity and real executor",
    ],
  };
  mkdirSync(new URL("../.local-ci-runs/", import.meta.url), {
    recursive: true,
  });
  writeFileSync(
    new URL("../.local-ci-runs/ai-provider-matrix.json", import.meta.url),
    JSON.stringify(receipt, null, 2) + "\n",
  );
  console.log("[a06] receipt: .local-ci-runs/ai-provider-matrix.json", verdict);
  process.exitCode = verdict.passed ? 0 : 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
