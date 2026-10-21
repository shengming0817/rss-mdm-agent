import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { checkCurrentContract } from "./ai-contract-evolution.mjs";
const schema = JSON.parse(
  readFileSync(
    new URL(
      "../packages/ai-contract/schema/runtime.schema.json",
      import.meta.url,
    ),
  ),
);
test("display extension does not change current contract identity", () => {
  const current = structuredClone(schema);
  current.$defs.Connection.properties.presentation.properties.icon = {
    type: "string",
  };
  checkCurrentContract(current);
});
test("all current records and the schema identity match negotiation", () => {
  const current = structuredClone(schema);
  current.$defs.SessionPage.properties.schemaVersion.const = 99;
  assert.throws(
    () => checkCurrentContract(current),
    /differs from negotiation/,
  );
  current.$defs.SessionPage.properties.schemaVersion.const = 1;
  current.$id = "urn:foreign";
  assert.throws(() => checkCurrentContract(current), /identity/);
});
