import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { expect, it } from "vitest";

it("every UI token reference has a scoped definition", () => {
  const root = resolve(import.meta.dirname, "../src");
  const tokens = readFileSync(resolve(root, "styles/tokens.css"), "utf8");
  const definitions = new Set(
    [...tokens.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]),
  );
  for (const file of readdirSync(root, {
    recursive: true,
    encoding: "utf8",
  }).filter((f) => /\.(vue|css)$/.test(f))) {
    const source = readFileSync(resolve(root, file), "utf8");
    for (const match of source.matchAll(/var\((--[\w-]+)/g))
      expect(definitions.has(match[1]), `${file}: ${match[1]}`).toBe(true);
  }
});
