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
import { cargoTargetDir } from "./cargo-target.mjs";
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
export function run(command, args, cwd, environment = process.env) {
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
    env: { ...environment, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0)
    throw new Error(
      `${command}: ${result.status ?? result.error?.code ?? result.signal}`,
    );
}
export function packHost(root, directory) {
  const names = [
    "ai-contract",
    "execution-bindings",
    "ai-store-sqlite",
    "platform-private-storage",
    "ai-host",
    "ai-access",
    "ai-adapter-claude",
    "ai-adapter-codex",
    "ai-adapter-deepseek",
    "ai-host-app",
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
  const { devDependencies: versions } = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  );
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
  const policy = load(readFileSync(join(root, "pnpm-workspace.yaml"), "utf8"));
  // pnpm 11's registry-policy verifier also sees file: tarballs. These local
  // artifacts have no registry publish date/provenance; their bytes are hashed below.
  const localExcludes = names.map((name) => {
    const { version } = JSON.parse(
      readFileSync(join(root, sourceDirectory(name), "package.json"), "utf8"),
    );
    return `@rss-mdm-agent/${name}@${version}`;
  });
  writeFileSync(
    join(directory, "pnpm-workspace.yaml"),
    JSON.stringify(
      {
        packages: [],
        overrides: dependencies,
        allowBuilds: policy.allowBuilds ?? {},
        minimumReleaseAgeExclude: [
          ...(policy.minimumReleaseAgeExclude ?? []),
          ...localExcludes,
        ],
        trustPolicyExclude: [
          ...(policy.trustPolicyExclude ?? []),
          ...localExcludes,
        ],
      },
      null,
      2,
    ),
  );
  return archives;
}

// ref: pnpm lockfile/types/src/index.ts@v11.4.0 (shared v9 lockfile format).
/** Graft only the packed workspace identities onto the source's locked graph. */
export function materializeDeploymentLock(root, directory) {
  const source = load(readFileSync(join(root, "pnpm-lock.yaml"), "utf8"));
  const manifest = JSON.parse(
    readFileSync(join(directory, "package.json"), "utf8"),
  );
  const local = Object.fromEntries(
    Object.entries(manifest.dependencies).map(([name, specifier]) => [
      name,
      specifier.replace(/^file:\.\//, "file:"),
    ]),
  );
  const importer = { dependencies: {}, devDependencies: {} };
  const deployed = {
    ...source,
    overrides: manifest.dependencies,
    importers: { ".": importer },
    packages: { ...source.packages },
    snapshots: { ...source.snapshots },
  };
  for (const [name, version] of Object.entries(local)) {
    assert.ok(
      name.startsWith("@rss-mdm-agent/") && version.startsWith("file:"),
    );
    const owner = sourceDirectory(name.slice("@rss-mdm-agent/".length));
    const pkg = JSON.parse(
      readFileSync(join(root, owner, "package.json"), "utf8"),
    );
    const locked = source.importers[owner];
    const edges = (dependencies) =>
      Object.fromEntries(
        Object.entries(dependencies ?? {}).map(([dependency, specifier]) => {
          if (local[dependency]) return [dependency, local[dependency]];
          assert.ok(
            !dependency.startsWith("@rss-mdm-agent/"),
            `missing packed dependency ${dependency}`,
          );
          const edge =
            locked.dependencies?.[dependency] ??
            locked.optionalDependencies?.[dependency] ??
            locked.devDependencies?.[dependency];
          assert.ok(edge, `unlocked direct edge ${name}: ${dependency}`);
          assert.equal(
            edge.specifier,
            specifier,
            `outdated source lock ${name}: ${dependency}`,
          );
          return [dependency, edge.version];
        }),
      );
    const key = `${name}@${version}`;
    const entry = {
      resolution: {
        integrity:
          "sha512-" +
          createHash("sha512")
            .update(
              readFileSync(join(directory, version.slice("file:".length))),
            )
            .digest("base64"),
        tarball: version,
      },
      version: pkg.version,
    };
    for (const field of [
      "engines",
      "peerDependencies",
      "peerDependenciesMeta",
      "os",
      "cpu",
      "libc",
    ])
      if (pkg[field]) entry[field] = pkg[field];
    if (pkg.bin) entry.hasBin = true;
    deployed.packages[key] = entry;
    deployed.snapshots[key] = {
      dependencies: edges({ ...pkg.dependencies, ...pkg.peerDependencies }),
      ...(pkg.optionalDependencies
        ? { optionalDependencies: edges(pkg.optionalDependencies) }
        : {}),
    };
    importer.dependencies[name] = { specifier: version, version };
  }
  for (const [name, specifier] of Object.entries(
    manifest.devDependencies ?? {},
  )) {
    const edge = source.importers["."].devDependencies?.[name];
    assert.ok(edge, `unlocked build dependency ${name}`);
    assert.equal(
      edge.specifier,
      specifier,
      `outdated source lock for build dependency ${name}`,
    );
    importer.devDependencies[name] = { ...edge };
  }
  writeFileSync(join(directory, "pnpm-lock.yaml"), JSON.stringify(deployed));
  return deployed;
}

/** Freeze the source graph across pnpm versions; hydrate missing cache data online. */
export function installArtifacts(root, directory) {
  materializeDeploymentLock(root, directory);
  run(
    "pnpm",
    [
      "install",
      "--prefer-offline",
      "--frozen-lockfile",
      ...(process.platform === "win32" ? ["--node-linker=hoisted"] : []),
      "--prod",
    ],
    directory,
  );
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

/** The canonical Node that shares a directory with both native runtime helpers. */
export function privateRuntimeNode(root) {
  return join(
    root,
    ".local-ci-runs/worker-runtime/bin",
    process.platform === "win32" ? "node.exe" : "node",
  );
}
export function runPrivateRuntime(root, args) {
  const result = spawnSync(privateRuntimeNode(root), args, {
    cwd: root,
    stdio: "inherit",
  });
  if (result.error) throw new Error("fixed private runtime unavailable");
  return result.status ?? 1;
}

/** Stage the fixed native launcher alongside the application Host. */
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
      join(cargoTargetDir(root), "release", name + suffix),
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
