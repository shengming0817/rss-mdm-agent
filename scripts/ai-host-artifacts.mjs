import { spawnSync } from "node:child_process";
import {
  lstatSync,
  mkdirSync,
  copyFileSync,
  chmodSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { createHash, timingSafeEqual } from "node:crypto";
import assert from "node:assert/strict";
import { load } from "js-yaml";
const runtimeRoots = [
  "bin",
  "node_modules",
  "NODE-LICENSE",
  "package.json",
  "pnpm-lock.yaml",
  "worker-manifest.json",
];
const sourceDirectory = (name) =>
  name === "ai-host-app"
    ? "apps/ai-host"
    : name.startsWith("ai-adapter-")
      ? `packages/ai-adapters/${name.slice("ai-adapter-".length)}`
      : `packages/${name}`;
export function run(command, args, cwd) {
  if (process.platform === "win32" && command === "pnpm") {
    const cli = process.env.npm_execpath;
    if (!cli || !/\.[cm]?js$/.test(cli))
      throw new Error(
        "Run Windows build tools from pnpm so its pinned CLI is explicit.",
      );
    args = [cli, ...args];
    command = process.execPath;
  }
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0)
    throw new Error(
      `${command}: ${result.status ?? result.error?.code ?? result.signal}`,
    );
}
export function packHost(root, directory, application = false) {
  const names = [
    "ai-contract",
    "ai-store-sqlite",
    "ai-host",
    ...(application
      ? [
          "ai-access",
          "ai-adapter-claude",
          "ai-adapter-codex",
          "ai-adapter-deepseek",
          "ai-host-app",
        ]
      : []),
  ];
  for (const name of names) {
    const source = sourceDirectory(name);
    run(
      "pnpm",
      ["--dir", join(root, source), "pack", "--pack-destination", directory],
      root,
    );
  }
  const archives = readdirSync(directory).filter((name) =>
    name.endsWith(".tgz"),
  );
  const dependencies = Object.fromEntries(
    names.map((name) => {
      const archive = archives.find((file) =>
        file.startsWith(`rss-mdm-agent-${name}-`),
      );
      if (!archive) throw new Error("missing archive");
      return [`@rss-mdm-agent/${name}`, `file:./${archive}`];
    }),
  );
  const artifacts = archives.map((name) => ({
    name,
    sha256: createHash("sha256")
      .update(readFileSync(join(directory, name)))
      .digest("hex"),
  }));
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify(
      {
        name: "isolated-ai-host",
        private: true,
        type: "module",
        dependencies,
        devDependencies: {
          typescript: versions.typescript,
          "@types/node": versions["@types/node"],
        },
      },
      null,
      2,
    ),
  );
  writeFileSync(
    join(directory, "pnpm-workspace.yaml"),
    JSON.stringify(
      {
        packages: [],
        overrides: dependencies,
        allowBuilds:
          load(readFileSync(join(root, "pnpm-workspace.yaml"), "utf8"))
            .allowBuilds ?? {},
        minimumReleaseAgeExclude:
          load(readFileSync(join(root, "pnpm-workspace.yaml"), "utf8"))
            .minimumReleaseAgeExclude ?? [],
      },
      null,
      2,
    ),
  );
  return artifacts;
}

/** Materialize a deployment lock, prove every edge against the source lock, then freeze install. */
export function installArtifacts(root, directory, production = false) {
  const source = load(readFileSync(join(root, "pnpm-lock.yaml"), "utf8"));
  // Seed pnpm with the locked external resolutions; only local tarball identities are new.
  writeFileSync(
    join(directory, "pnpm-lock.yaml"),
    JSON.stringify({ ...source, importers: {} }),
  );
  run("pnpm", ["install", "--offline", "--lockfile-only"], directory);
  const lockPath = join(directory, "pnpm-lock.yaml"),
    deployed = load(readFileSync(lockPath, "utf8"));
  for (const [key, entry] of Object.entries(deployed.packages)) {
    if (key.startsWith("@rss-mdm-agent/")) continue;
    assert.deepEqual(
      entry.resolution,
      source.packages[key]?.resolution,
      `unlocked package ${key}`,
    );
  }
  for (const [key, entry] of Object.entries(deployed.snapshots)) {
    if (!key.startsWith("@rss-mdm-agent/")) {
      for (const field of ["dependencies", "optionalDependencies"])
        assert.deepEqual(
          entry[field] ?? {},
          source.snapshots[key]?.[field] ?? {},
          `unlocked dependency graph ${key}`,
        );
      continue;
    }
    const name = key.slice("@rss-mdm-agent/".length).split("@file:")[0];
    const manifest = JSON.parse(
      readFileSync(join(root, sourceDirectory(name), "package.json"), "utf8"),
    );
    const importer = source.importers[sourceDirectory(name)];
    const expected = { ...manifest.dependencies, ...manifest.peerDependencies };
    assert.deepEqual(
      Object.keys(entry.dependencies ?? {}).sort(),
      Object.keys(expected).sort(),
      `unexpected package edge ${name}`,
    );
    for (const [dependency, version] of Object.entries(
      entry.dependencies ?? {},
    )) {
      if (dependency.startsWith("@rss-mdm-agent/"))
        assert.equal(
          version,
          deployed.importers["."].dependencies[dependency]?.version,
          `unlocked local edge ${name}`,
        );
      else
        assert.equal(
          version,
          (
            importer.dependencies?.[dependency] ??
            importer.devDependencies?.[dependency]
          )?.version,
          `unlocked direct edge ${name}`,
        );
    }
  }
  run(
    "pnpm",
    [
      "install",
      "--offline",
      "--frozen-lockfile",
      ...(process.platform === "win32" ? ["--node-linker=hoisted"] : []),
      ...(production ? ["--prod"] : []),
    ],
    directory,
  );
  return createHash("sha256").update(readFileSync(lockPath)).digest("hex");
}

function hashRuntimeEntry(hash, root, name) {
  const path = join(root, name),
    stat = lstatSync(path),
    portableName = name.split(sep).join("/");
  if (stat.isSymbolicLink()) {
    const target = readlinkSync(path),
      resolved = resolve(dirname(path), target),
      fromRoot = relative(root, resolved);
    if (
      isAbsolute(target) ||
      isAbsolute(fromRoot) ||
      fromRoot === ".." ||
      fromRoot.startsWith(`..${sep}`)
    )
      throw new Error(
        `runtime symlink escapes deployment tree: ${portableName}`,
      );
    hash.update(`link\0${portableName}\0${target}\0`);
    return;
  }
  if (stat.isDirectory()) {
    hash.update(`directory\0${portableName}\0`);
    for (const child of readdirSync(path).sort())
      hashRuntimeEntry(hash, root, join(name, child));
    return;
  }
  if (!stat.isFile())
    throw new Error(`unsupported runtime entry: ${portableName}`);
  hash.update(
    `file\0${portableName}\0${process.platform === "win32" ? 0 : stat.mode & 0o777}\0${stat.size}\0`,
  );
  hash.update(readFileSync(path));
}

/** Hash the complete deployed runtime tree without embedding its local absolute path. */
export function runtimeTreeSha256(directory) {
  const hash = createHash("sha256");
  for (const name of runtimeRoots) hashRuntimeEntry(hash, directory, name);
  return hash.digest("hex");
}

/** Recompute and verify the deployed bytes, file modes and pnpm symlink graph. */
export function verifyRuntimeIntegrity(directory, expectedSha256) {
  if (!/^[a-f0-9]{64}$/.test(expectedSha256 ?? ""))
    throw new Error("runtime tree integrity digest is missing or invalid");
  const actual = runtimeTreeSha256(directory);
  if (
    !timingSafeEqual(
      Buffer.from(actual, "hex"),
      Buffer.from(expectedSha256, "hex"),
    )
  )
    throw new Error("runtime tree integrity mismatch");
  return actual;
}

/** Root manifest owns the runtime version; each approved archive binds its platform and SQLite ABI. */
export function runtimeArtifact(root, target) {
  const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8"))
    .engines.node;
  const artifacts = {
    "24.14.1/darwin-arm64": {
      sha256:
        "25495ff85bd89e2d8a24d88566d7e2f827c6b0d3d872b2cebf75371f93fcb1fe",
      sqlite: "3.51.2",
    },
  };
  artifacts["24.14.1/win32-x64"] = {
    sha256: "6e50ce5498c0cebc20fd39ab3ff5df836ed2f8a31aa093cecad8497cff126d70",
    sqlite: "3.51.2",
  };
  const artifact = artifacts[`${version}/${target}`];
  if (!artifact)
    throw new Error(`No verified Node artifact for ${version}/${target}`);
  return {
    version,
    target: target === "win32-x64" ? "win-x64" : target,
    ...artifact,
  };
}

/** Stage the fixed native launcher alongside a packaged Host, including isolated consumers. */
export function stageWorkerRuntime(
  root,
  directory,
  node = process.execPath,
  bootstrap = "node_modules/@rss-mdm-agent/ai-host/dist/bootstrap.js",
) {
  const suffix = process.platform === "win32" ? ".exe" : "";
  mkdirSync(join(directory, "bin"), { recursive: true });
  const runtimeNode = join(directory, "bin/node" + suffix);
  if (resolve(node) !== resolve(runtimeNode)) copyFileSync(node, runtimeNode);
  for (const name of ["rss-ai-worker-launcher", "rss-private-storage"]) {
    copyFileSync(
      join(root, "target/release", name + suffix),
      join(directory, "bin", name + suffix),
    );
    if (process.platform !== "win32")
      chmodSync(join(directory, "bin", name + suffix), 0o755);
  }
  const hash = (path) =>
    createHash("sha256")
      .update(readFileSync(join(directory, path)))
      .digest("hex");
  writeFileSync(
    join(directory, "worker-manifest.json"),
    JSON.stringify({
      version: 1,
      node: "bin/node" + suffix,
      bootstrap,
      node_sha256: hash("bin/node" + suffix),
      bootstrap_sha256: hash(bootstrap),
    }) + "\n",
  );
}
