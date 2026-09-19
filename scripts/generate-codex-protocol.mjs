import {
  mkdtempSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  rmSync,
  readdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { format } from "prettier";
const root = fileURLToPath(
  new URL("../packages/ai-adapters/codex/", import.meta.url),
);
const require = createRequire(join(root, "package.json"));
const manifest = require("@openai/codex/package.json");
if (manifest.version !== "0.155.0") throw new Error("Codex version drift");
const cli = join(
  dirname(require.resolve("@openai/codex/package.json")),
  "bin/codex.js",
);
const temp = mkdtempSync(join(tmpdir(), "rss-codex-protocol-"));
const roots = [
  "InitializeParams",
  "InitializeResponse",
  ...[
    "ThreadStartParams",
    "ThreadStartResponse",
    "ThreadResumeParams",
    "ThreadResumeResponse",
    "ThreadForkParams",
    "ThreadForkResponse",
    "ThreadReadParams",
    "ThreadReadResponse",
    "ThreadTurnsListParams",
    "ThreadTurnsListResponse",
    "ThreadItemsListParams",
    "ThreadItemsListResponse",
    "TurnStartParams",
    "TurnStartResponse",
    "TurnSteerParams",
    "TurnSteerResponse",
    "TurnInterruptParams",
    "ListMcpServerStatusParams",
    "ListMcpServerStatusResponse",
    "ConfigReadParams",
    "ConfigReadResponse",
    "AgentMessageDeltaNotification",
    "TurnCompletedNotification",
    "ItemCompletedNotification",
  ].map((name) => `v2/${name}`),
];
try {
  const result = spawnSync(
    process.execPath,
    [cli, "app-server", "generate-ts", "--experimental", "--out", temp],
    { encoding: "utf8" },
  );
  if (result.status !== 0) throw new Error("Pinned protocol generation failed");
  const outputs = new Map();
  async function add(name) {
    if (outputs.has(name)) return;
    const source = readFileSync(join(temp, name), "utf8");
    outputs.set(name, "");
    for (const [, dependency] of source.matchAll(/from "([^"]+)"/g))
      await add(
        relative(temp, resolve(temp, dirname(name), dependency + ".ts")),
      );
    outputs.set(
      name,
      await format(source.replace(/from "([^"]+)"/g, 'from "$1.js"'), {
        parser: "typescript",
      }),
    );
  }
  for (const name of roots) await add(`${name}.ts`);
  const destination = join(root, "src/protocol");
  const hashes = {};
  for (const [name, source] of [...outputs].sort(([a], [b]) =>
    a.localeCompare(b),
  )) {
    const path = join(destination, name);
    hashes[name] = createHash("sha256").update(source).digest("hex");
    if (process.argv.includes("--check")) {
      if (readFileSync(path, "utf8") !== source)
        throw new Error(`Codex schema drift: ${name}`);
    } else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, source);
    }
  }
  const walk = (directory) =>
    readdirSync(directory, { withFileTypes: true }).flatMap((e) =>
      e.isDirectory()
        ? walk(join(directory, e.name))
        : [relative(destination, join(directory, e.name))],
    );
  if (walk(destination).some((name) => !outputs.has(name)))
    throw new Error("Unexpected generated protocol file");
  const provenance =
    JSON.stringify(
      {
        runtime: manifest.version,
        revision: "f0a1b8f0849d90960bc406b848f32e5a129b0457",
        experimentalApi: true,
        experimentalUsage:
          "Explicitly empty environments, runtimeWorkspaceRoots and dynamicTools for containment; dynamic tools are unsupported",
        transformation:
          "generate-ts dependency closure; NodeNext import suffixes and formatting only",
        sha256: hashes,
      },
      null,
      2,
    ) + "\n";
  const path = join(root, "protocol-manifest.json");
  if (process.argv.includes("--check")) {
    if (readFileSync(path, "utf8") !== provenance)
      throw new Error("Codex provenance drift");
  } else writeFileSync(path, provenance);
  console.log(
    `Codex ${manifest.version}: ${outputs.size} generated protocol types verified`,
  );
} finally {
  rmSync(temp, { recursive: true, force: true });
}
