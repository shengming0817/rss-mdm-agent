import {
  mkdtempSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  rmSync,
  readdirSync,
  renameSync,
  existsSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { format } from "prettier";
async function main() {
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
      "LoginAccountParams",
      "LoginAccountResponse",
      "GetAccountParams",
      "GetAccountResponse",
      "ChatgptAuthTokensRefreshParams",
      "ChatgptAuthTokensRefreshResponse",
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
      "ItemStartedNotification",
    ].map((name) => `v2/${name}`),
  ];
  try {
    const result = spawnSync(
      process.execPath,
      [cli, "app-server", "generate-ts", "--experimental", "--out", temp],
      { encoding: "utf8" },
    );
    if (result.status !== 0)
      throw new Error("Pinned protocol generation failed");
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
    ))
      hashes[name] = createHash("sha256").update(source).digest("hex");
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
    publishProtocol(
      destination,
      join(root, "protocol-manifest.json"),
      outputs,
      provenance,
      process.argv.includes("--check"),
    );
    console.log(
      `Codex ${manifest.version}: ${outputs.size} generated protocol types verified`,
    );
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

/** Stage the complete closure and manifest before touching the managed output.
 * A failed publication restores the previous tree and manifest. Check never writes. */
export function publishProtocol(
  destination,
  manifestPath,
  outputs,
  provenance,
  check = false,
) {
  const files = (directory, base = directory) =>
    existsSync(directory)
      ? readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
          entry.isDirectory()
            ? files(join(directory, entry.name), base)
            : [relative(base, join(directory, entry.name))],
        )
      : [];
  for (const name of outputs.keys()) {
    const path = relative(destination, resolve(destination, name));
    if (!path || path.startsWith("..") || path !== name)
      throw new Error("Invalid generated path");
  }
  if (check) {
    if (
      JSON.stringify(files(destination).sort()) !==
      JSON.stringify([...outputs.keys()].sort())
    )
      throw new Error("Codex generated file set drift");
    for (const [name, source] of outputs)
      if (readFileSync(join(destination, name), "utf8") !== source)
        throw new Error(`Codex schema drift: ${name}`);
    if (readFileSync(manifestPath, "utf8") !== provenance)
      throw new Error("Codex provenance drift");
    return;
  }
  const stage = mkdtempSync(join(dirname(destination), ".protocol-stage-"));
  const tree = join(stage, "new"),
    backup = join(stage, "old");
  let moved = false,
    installed = false;
  try {
    mkdirSync(tree);
    for (const [name, source] of outputs) {
      mkdirSync(dirname(join(tree, name)), { recursive: true });
      writeFileSync(join(tree, name), source);
    }
    writeFileSync(join(stage, "manifest.json"), provenance);
    // Read/validate the old manifest before replacing any managed output.
    if (existsSync(manifestPath)) readFileSync(manifestPath, "utf8");
    if (existsSync(destination)) {
      renameSync(destination, backup);
      moved = true;
    }
    renameSync(tree, destination);
    installed = true;
    renameSync(join(stage, "manifest.json"), manifestPath);
  } catch (error) {
    if (installed) rmSync(destination, { recursive: true, force: true });
    if (moved) renameSync(backup, destination);
    throw error;
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
