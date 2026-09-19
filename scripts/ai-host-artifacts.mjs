import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { load } from "js-yaml";
const sourceDirectory = (name) =>
  name === "ai-host-app"
    ? "apps/ai-host"
    : name === "ai-adapter-claude"
      ? "packages/ai-adapters/claude"
      : `packages/${name}`;
export function run(command, args, cwd) {
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
    ...(application ? ["ai-access", "ai-adapter-claude", "ai-host-app"] : []),
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
export function installHost(root, directory, production = false) {
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
      ...(production ? ["--prod"] : []),
    ],
    directory,
  );
  return createHash("sha256").update(readFileSync(lockPath)).digest("hex");
}
