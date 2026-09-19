import assert from "node:assert/strict";
import test from "node:test";
import { readFile, readdir } from "node:fs/promises";
import { HARNESS_VERSION } from "../../../packages/ai-adapters/deepseek/dist/configuration.js";
test("manifest freezes only source import roots and derives the Harness identity", async () => {
  const root = new URL(
    "../../../packages/ai-adapters/deepseek/",
    import.meta.url,
  );
  const manifest = JSON.parse(
      await readFile(new URL("package.json", root), "utf8"),
    ),
    imports = new Set();
  for (const name of await readdir(new URL("src/", root))) {
    const text = await readFile(new URL(`src/${name}`, root), "utf8");
    for (const [, specifier] of text.matchAll(
      /(?:from\s+|import\s*)["']([^"']+)["']/g,
    )) {
      if (specifier.startsWith(".") || specifier.startsWith("node:")) continue;
      imports.add(
        specifier.startsWith("@")
          ? specifier.split("/").slice(0, 2).join("/")
          : specifier.split("/")[0],
      );
    }
  }
  assert.deepEqual(
    Object.keys(manifest.dependencies).sort(),
    [...imports].sort(),
  );
  assert.equal(
    HARNESS_VERSION,
    manifest.dependencies["@deepseek-ai/dsh-api-session-controller"],
  );
  for (const [name, version] of Object.entries(manifest.dependencies)) {
    if (name.startsWith("@deepseek-ai/dsh-"))
      assert.equal(version, HARNESS_VERSION);
    if (!name.startsWith("@rss-mdm-agent/"))
      assert.match(version, /^\d+\.\d+\.\d+(?:-[\w.]+)?$/);
  }
});
