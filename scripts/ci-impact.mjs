// ref: RSS hack/ci-impact.py; package closure with explicit product source seams.
import { execFileSync } from "node:child_process";
import { readFileSync, realpathSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
const git = process.platform === "win32" ? "git" : "/usr/bin/git";
const npm = (name) => `@rss-mdm-agent/${name}`;
const run = (root, command, args) =>
  execFileSync(command, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
const rootDocs = new Set([
  "README.md",
  "AGENTS.md",
  "CLAUDE.md",
  "CHANGELOG.md",
  "LICENSE",
  "LICENSE.md",
  "SECURITY.md",
  "CONTRIBUTING.md",
]);
const globalFiles = new Set([
  "Makefile",
  "Cargo.lock",
  "Cargo.toml",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "rust-toolchain.toml",
  ".npmrc",
  ".node-version",
  ".nvmrc",
  "tsconfig.json",
  "vitest.config.ts",
]);
const globalPath = (path) =>
  globalFiles.has(path) ||
  /(^|\/)(Cargo.toml|package.json|tsconfig[^/]*\.json)$/.test(path) ||
  ["scripts/", ".cargo/", ".config/", ".github/", "schemas/"].some((prefix) =>
    path.startsWith(prefix),
  );
const documentation = (path) =>
  rootDocs.has(path) ||
  path.startsWith("docs/") ||
  (path.endsWith(".md") &&
    [".claude/skills/", ".codex/skills/"].some((prefix) =>
      path.startsWith(prefix),
    ));

export function parseChanges(raw) {
  const fields = raw.split("\0");
  if (fields.pop() !== "") throw Error("invalid-diff");
  const changes = [];
  while (fields.length) {
    const status = fields.shift();
    if (/^[RC]/.test(status)) throw Error("rename-or-copy");
    const path = fields.shift();
    if (
      !/^[ADM]$/.test(status) ||
      !path ||
      path.startsWith("/") ||
      path.split("/").includes("..")
    )
      throw Error("invalid-diff");
    changes.push([status, path]);
  }
  return changes;
}

// Build scripts, generated bindings, runtime packing and shared fixtures are not
// represented by Cargo or pnpm dependencies. These are directed consumer edges.
export const sourceEdges = [
  [npm("ai-contract"), "ai-session-contract"],
  ["ai-session-contract", npm("ai-contract")],
  ["execution-app", npm("desktop")],
  ["execution-app", npm("ai-host-app")],
  ["execution-mcp", npm("ai-host-app")],
  ["rss-mdm-desktop", npm("desktop")],
  [npm("desktop"), "rss-mdm-desktop"],
  [npm("ai-host-app"), "rss-mdm-desktop"],
  [npm("ai-client"), npm("ai-host-app")],
];
export const testOwners = [
  ["apps/ai-host/src/execution-tools.json", "execution-mcp"],
  ["apps/desktop/src/assistant/execution-types.ts", "execution-app"],
  ["tests/assistant/execution-fixtures.json", "execution-app"],
  ["tests/ai-contract", npm("ai-contract")],
  ["tests/ai-access", npm("ai-access")],
  ["tests/ai-access", npm("ai-client")],
  ["tests/ai-access", npm("ai-ui-bridge")],
  ["tests/ai-store-recovery", npm("ai-store-sqlite")],
  ["tests/ai-adapters/claude", npm("ai-adapter-claude")],
  ["tests/ai-adapters/codex", npm("ai-adapter-codex")],
  ["tests/ai-adapters/deepseek", npm("ai-adapter-deepseek")],
  ["tests/ai-host", npm("ai-host-app")],
  ["tests/ai-provider-conformance", npm("ai-host-app")],
  ...["claude", "codex", "deepseek"].map((name) => [
    "tests/ai-provider-conformance",
    npm(`ai-adapter-${name}`),
  ]),
  ["tests/ai-recovery-integration", npm("ai-host-app")],
  ["tests/assistant", npm("desktop")],
  ["tests/desktop", npm("desktop")],
  ["tests/execution-sqlite", "execution-sqlite"],
];

export function workspaceGraph(
  root,
  cargo,
  packages,
  edges = sourceEdges,
  owners = testOwners,
) {
  root = resolve(root);
  if (resolve(cargo.workspace_root) !== root)
    throw Error("workspace-root-mismatch");
  const roots = [],
    reverse = new Map(),
    rust = new Set(),
    node = new Set();
  const members = new Set(cargo.workspace_members),
    ids = new Map();
  const add = (name, directory, language) => {
    const path = relative(root, directory).split("\\").join("/");
    if (!path || path.startsWith("../") || reverse.has(name))
      throw Error("invalid-package-root");
    roots.push([path, name]);
    reverse.set(name, new Set());
    language.add(name);
  };
  for (const p of cargo.packages)
    if (members.has(p.id)) {
      add(p.name, dirname(p.manifest_path), rust);
      ids.set(p.id, p.name);
    }
  if (ids.size !== members.size) throw Error("missing-workspace-package");
  const seen = new Set();
  for (const entry of cargo.resolve.nodes) {
    if (seen.has(entry.id)) throw Error("duplicate-resolve-node");
    seen.add(entry.id);
    if (!ids.has(entry.id)) continue;
    for (const dep of entry.deps)
      if (ids.has(dep.pkg))
        reverse.get(ids.get(dep.pkg)).add(ids.get(entry.id));
  }
  if ([...members].some((id) => !seen.has(id)))
    throw Error("missing-resolve-node");
  for (const p of packages) add(p.name, p.path, node);
  for (const p of packages)
    for (const section of [
      "dependencies",
      "devDependencies",
      "peerDependencies",
      "optionalDependencies",
    ]) {
      for (const [name, version] of Object.entries(p.manifest[section] ?? {})) {
        if (node.has(name)) reverse.get(name).add(p.name);
        else if (version.startsWith("workspace:"))
          throw Error("unknown-workspace-dependency");
      }
    }
  for (const [dependency, consumer] of edges) {
    if (!reverse.has(dependency) || !reverse.has(consumer))
      throw Error("unmapped-source-edge");
    reverse.get(dependency).add(consumer);
  }
  for (const [path, name] of owners) {
    if (!reverse.has(name)) {
      if (edges.length) throw Error("unmapped-test-owner");
      continue;
    }
    roots.push([path, name]);
  }
  roots.sort((a, b) => b[0].length - a[0].length);
  return { roots, reverse, rust, node };
}

export function loadGraph(root) {
  const cargo = JSON.parse(
    run(root, "cargo", [
      "metadata",
      "--locked",
      "--all-features",
      "--format-version",
      "1",
    ]),
  );
  const packages = JSON.parse(
    run(root, "pnpm", ["list", "-r", "--depth", "-1", "--json"]),
  )
    .filter((p) => realpathSync(p.path) !== realpathSync(root))
    .map((p) => ({
      ...p,
      manifest: JSON.parse(
        readFileSync(resolve(p.path, "package.json"), "utf8"),
      ),
    }));
  if (!packages.length) throw Error("empty-node-workspace");
  return workspaceGraph(realpathSync(root), cargo, packages);
}

export function selectImpact(
  root,
  {
    baseRef = process.env.CI_BASE || "origin/develop",
    full = process.env.CI_FULL === "1",
    graph = loadGraph,
  } = {},
) {
  const decision = {
    full: true,
    packages: [],
    rustPackages: [],
    nodePackages: [],
    reasons: [],
    baseRef,
  };
  try {
    decision.head = run(root, git, ["rev-parse", "HEAD"]).trim();
    if (full) {
      decision.reasons = ["explicit-full"];
      return decision;
    }
    const branch = run(root, git, ["branch", "--show-current"]).trim();
    if (!branch || branch === "develop") {
      decision.reasons = [branch ? "develop" : "detached-head"];
      return decision;
    }
    decision.base = run(root, git, [
      "merge-base",
      baseRef,
      decision.head,
    ]).trim();
    const changes = parseChanges(
      run(root, git, [
        "diff",
        "--name-status",
        "-z",
        "--find-renames",
        "--find-copies",
        "--find-copies-harder",
        decision.base,
        "--",
      ]),
    );
    for (const path of run(root, git, [
      "ls-files",
      "--others",
      "--exclude-standard",
      "-z",
    ])
      .split("\0")
      .filter(Boolean)) {
      changes.push(["A", path]);
    }
    if (changes.some(([, path]) => globalPath(path))) {
      decision.reasons = ["global-input"];
      return decision;
    }
    const code = changes.filter(([, path]) => !documentation(path));
    if (!code.length)
      return {
        ...decision,
        full: false,
        reasons: [changes.length ? "docs-only" : "no-changes"],
      };
    const data = graph(root),
      selected = new Set();
    for (const [, path] of code) {
      const owners = data.roots.filter(
        ([dir]) => path === dir || path.startsWith(`${dir}/`),
      );
      if (!owners.length) throw Error("unknown-path");
      // A nested Cargo root wins over its containing npm package; equal roots may
      // deliberately map one shared test directory to multiple public consumers.
      const longest = Math.max(...owners.map(([dir]) => dir.length));
      for (const [dir, name] of owners)
        if (dir.length === longest) selected.add(name);
    }
    const todo = [...selected];
    while (todo.length)
      for (const consumer of data.reverse.get(todo.pop()) ?? []) {
        if (!selected.has(consumer)) {
          selected.add(consumer);
          todo.push(consumer);
        }
      }
    return {
      ...decision,
      full: false,
      packages: [...selected].sort(),
      rustPackages: [...selected].filter((p) => data.rust.has(p)).sort(),
      nodePackages: [...selected].filter((p) => data.node.has(p)).sort(),
      reasons: ["package-change"],
    };
  } catch (error) {
    return {
      ...decision,
      full: true,
      packages: [],
      rustPackages: [],
      nodePackages: [],
      reasons: [`selection-unavailable: ${error.message}`],
    };
  }
}
