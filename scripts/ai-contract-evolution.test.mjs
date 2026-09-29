import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { checkContractEvolution } from "./ai-contract-evolution.mjs";
const schema = JSON.parse(
  readFileSync(
    new URL(
      "../packages/ai-contract/schema/runtime.schema.json",
      import.meta.url,
    ),
  ),
);
test("session response replacement needs a contract version change", () => {
  const changed = structuredClone(schema);
  changed.$defs.SessionPage.properties.items.items.$ref = "#/$defs/Session";
  assert.throws(
    () => checkContractEvolution(schema, changed),
    /version increase/,
  );
  const next = schema.$defs.Negotiation.properties.contractVersion.const + 1;
  function upgrade(node) {
    if (!node || typeof node !== "object") return;
    for (const key of ["schemaVersion", "contractVersion"])
      if (node.properties?.[key]) node.properties[key].const = next;
    for (const value of Object.values(node)) upgrade(value);
  }
  upgrade(changed);
  changed.$id = `urn:rss-mdm-agent:ai-runtime:${next}`;
  checkContractEvolution(schema, changed);
  changed.$defs.SessionPage.properties.schemaVersion.const--;
  assert.throws(
    () => checkContractEvolution(schema, changed),
    /differs from negotiation/,
  );
});
test("documentation edits need no contract version change", () => {
  const changed = structuredClone(schema);
  changed.$defs.SessionPage.description = "Reworded documentation";
  checkContractEvolution(schema, changed);
});
