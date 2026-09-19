import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
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
    const source =
      name === "ai-host-app"
        ? "apps/ai-host"
        : name === "ai-adapter-claude"
          ? "packages/ai-adapters/claude"
          : `packages/${name}`;
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
        pnpm: { overrides: dependencies },
      },
      null,
      2,
    ),
  );
  writeFileSync(join(directory, "pnpm-workspace.yaml"), "packages: []\n");
  return artifacts;
}
