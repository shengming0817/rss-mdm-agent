import { execFileSync } from "node:child_process";

// Conservative product gate, not a general JSON Schema compatibility engine:
// every structural change requires a new exact contract version. Documentation
// and object-key ordering do not change the contract.
const documentation = new Set(["description", "title", "$comment", "examples"]);
function structure(value, schema = true) {
  if (Array.isArray(value))
    return value.map((child) => structure(child, schema));
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, child]) =>
          !schema ||
          !documentation.has(key) ||
          (typeof child !== "string" && !Array.isArray(child)),
      )
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, child]) => [
        key,
        structure(child, schema && !["const", "enum", "default"].includes(key)),
      ]),
  );
}
export function checkContractEvolution(base, current) {
  const previous = base.$defs.Negotiation.properties.contractVersion.const;
  const next = current.$defs.Negotiation.properties.contractVersion.const;
  if (!Number.isSafeInteger(next) || next < previous)
    throw new Error("AI contract version must not regress");
  if (
    JSON.stringify(structure(base)) !== JSON.stringify(structure(current)) &&
    next <= previous
  )
    throw new Error("AI contract structure changed without a version increase");
  function records(node) {
    if (!node || typeof node !== "object") return;
    if (
      node.properties?.schemaVersion &&
      node.properties.schemaVersion.const !== next
    )
      throw new Error("AI record version differs from negotiation");
    for (const child of Object.values(node)) records(child);
  }
  records(current);
  if (current.$id !== `urn:rss-mdm-agent:ai-runtime:${next}`)
    throw new Error("AI schema identity differs from negotiation");
}
export function checkContractBase(root, current) {
  const git = (...args) =>
    execFileSync(process.platform === "win32" ? "git" : "/usr/bin/git", args, {
      cwd: root,
      encoding: "utf8",
    }).trim();
  const base = git(
    "merge-base",
    process.env.CI_BASE || "origin/develop",
    "HEAD",
  );
  checkContractEvolution(
    JSON.parse(
      git("show", `${base}:packages/ai-contract/schema/runtime.schema.json`),
    ),
    current,
  );
}
