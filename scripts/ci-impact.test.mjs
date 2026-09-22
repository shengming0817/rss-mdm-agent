import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  selectImpact,
  workspaceGraph,
  parseChanges,
  ciSourceState,
} from "./ci-impact.mjs";

const npm = (name) => `@rss-mdm-agent/${name}`;
function fixture() {
  const root = mkdtempSync(join(tmpdir(), "agent-impact-"));
  const run = (...args) =>
    execFileSync("/usr/bin/git", args, { cwd: root, encoding: "utf8" }).trim();
  const write = (file, text = `${file}\n`) => {
    mkdirSync(join(root, file, ".."), { recursive: true });
    writeFileSync(join(root, file), text);
  };
  run("init", "-q");
  run("config", "user.name", "Test");
  run("config", "user.email", "test@example.invalid");
  write("README.md");
  write("crates/core/src/lib.rs");
  run("add", ".");
  run("commit", "-qm", "base");
  const base = run("rev-parse", "HEAD");
  run("checkout", "-qb", "topic");
  const graph = {
    roots: [
      ["crates/core", "core"],
      ["crates/leaf", "leaf"],
      ["packages/ui", npm("ui")],
    ],
    reverse: new Map([
      ["core", new Set(["leaf"])],
      ["leaf", new Set()],
      [npm("ui"), new Set()],
    ]),
    rust: new Set(["core", "leaf"]),
    node: new Set([npm("ui")]),
  };
  return {
    root,
    run,
    write,
    base,
    graph,
    change(file, text) {
      write(file, text);
      run("add", ".");
      run("commit", "-qm", "change");
    },
    select(extra = {}) {
      return selectImpact(root, {
        baseRef: base,
        graph: () => graph,
        ...extra,
      });
    },
    close() {
      rmSync(root, { recursive: true, force: true });
    },
  };
}

test("docs-only and mixed changes use the code reverse closure", () => {
  const f = fixture();
  try {
    assert.equal(f.select().reasons[0], "no-changes");
    f.change("docs/guide.md");
    assert.deepEqual(f.select().packages, []);
    f.change("crates/core/src/lib.rs", "modified\n");
    assert.deepEqual(f.select().rustPackages, ["core", "leaf"]);
    assert.equal(f.select().full, false);
  } finally {
    f.close();
  }
});

test("global, unknown, dirty, invalid base, develop and explicit full fail full", () => {
  for (const path of [
    "Cargo.lock",
    "packages/ui/package.json",
    "scripts/ci.mjs",
    "unowned/input",
    "docs/example/Cargo.toml",
  ]) {
    const f = fixture();
    try {
      f.change(path);
      assert.equal(f.select().full, true, path);
    } finally {
      f.close();
    }
  }
  const f = fixture();
  try {
    assert.equal(f.select({ baseRef: "missing" }).full, true);
    assert.equal(f.select({ full: true }).reasons[0], "explicit-full");
    f.run("branch", "-m", "develop");
    assert.equal(f.select().reasons[0], "develop");
    f.run("branch", "-m", "topic");
    f.write("untracked");
    assert.equal(f.select().reasons[0], "dirty-input");
  } finally {
    f.close();
  }
});

test("renames, copies and malformed diff never shrink scope", () => {
  for (const raw of [
    "R100\0old\0new\0",
    "C100\0old\0new\0",
    "T\0file\0",
    "M\0../escape\0",
    "M\0",
  ]) {
    assert.throws(() => parseChanges(raw));
  }
  const f = fixture();
  try {
    f.run("mv", "README.md", "guide.md");
    f.run("commit", "-qm", "rename");
    assert.equal(f.select().full, true);
  } finally {
    f.close();
  }
});

test("metadata failures and unowned deletions fail full", () => {
  const f = fixture();
  try {
    f.change("crates/core/src/lib.rs", "modified\n");
    assert.equal(
      f.select({
        graph() {
          throw Error("metadata failure");
        },
      }).full,
      true,
    );
    f.change("old-file");
    const baseRef = f.run("rev-parse", "HEAD");
    f.run("rm", "old-file");
    f.run("commit", "-qm", "delete");
    assert.equal(f.select({ baseRef }).full, true);
  } finally {
    f.close();
  }
});

test("Cargo normal/dev/build/optional edges and pnpm dependencies form reverse closure", () => {
  const root = "/fixture";
  const names = ["core", "normal", "dev", "build", "optional"];
  const cargo = {
    workspace_root: root,
    workspace_members: names,
    packages: names.map((name) => ({
      id: name,
      name,
      manifest_path: `${root}/crates/${name}/Cargo.toml`,
    })),
    resolve: {
      nodes: names.map((id) => ({
        id,
        deps: id === "core" ? [] : [{ pkg: "core" }],
      })),
    },
  };
  const packages = [
    { name: npm("ui"), path: `${root}/packages/ui`, manifest: {} },
    {
      name: npm("desktop"),
      path: `${root}/apps/desktop`,
      manifest: { dependencies: { [npm("ui")]: "workspace:*" } },
    },
  ];
  const graph = workspaceGraph(root, cargo, packages, []);
  assert.deepEqual([...graph.reverse.get("core")].sort(), [
    "build",
    "dev",
    "normal",
    "optional",
  ]);
  assert.deepEqual([...graph.reverse.get(npm("ui"))], [npm("desktop")]);
  cargo.resolve.nodes.pop();
  assert.throws(() => workspaceGraph(root, cargo, packages, []));
});

test("explicit cross-language inputs and external test owners remain in closure", () => {
  const f = fixture();
  try {
    f.graph.roots.push(["tests/ai-contract", npm("ai-contract")]);
    f.graph.node.add(npm("ai-contract"));
    f.graph.reverse.set(npm("ai-contract"), new Set(["core"]));
    f.change("tests/ai-contract/codec.test.mjs");
    assert.deepEqual(f.select().rustPackages, ["core", "leaf"]);
    assert.deepEqual(f.select().nodePackages, [npm("ai-contract")]);
  } finally {
    f.close();
  }
});

test("deepest package ownership and known file deletion remain selective", () => {
  const f = fixture();
  try {
    f.graph.roots.push(["crates/core/nested", "leaf"]);
    f.change("crates/core/nested/lib.rs", "nested only");
    assert.deepEqual(f.select().rustPackages, ["leaf"]);
    const baseRef = f.run("rev-parse", "HEAD");
    f.run("rm", "crates/core/src/lib.rs");
    f.run("commit", "-qm", "delete known source");
    assert.deepEqual(f.select({ baseRef }).rustPackages, ["core", "leaf"]);
  } finally {
    f.close();
  }
});

test("detached HEAD is conservative and missing baseline retains the real HEAD", () => {
  const f = fixture();
  try {
    f.run("checkout", "--detach");
    assert.equal(f.select().full, true);
    const state = ciSourceState(f.root, "missing-ref");
    assert.equal(state.head, f.run("rev-parse", "HEAD"));
    assert.equal(state.baseRef, "missing-ref");
    assert.equal(state.clean, false);
  } finally {
    f.close();
  }
});
