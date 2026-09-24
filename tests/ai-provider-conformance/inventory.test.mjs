import assert from "node:assert/strict";
import test from "node:test";
import { nativeToolInventory } from "./support.mjs";
test("native tool inventory is extracted from all observed protocol requests", () => {
  assert.deepEqual(
    nativeToolInventory([
      {
        tools: [
          { name: "AskUserQuestion" },
          { type: "function", function: { name: "ask_user_question" } },
        ],
      },
      {
        tools: [
          {
            type: "namespace",
            name: "mcp__rss_host",
            tools: [{ name: "propose" }],
          },
          { name: "AskUserQuestion" },
        ],
      },
    ]),
    ["AskUserQuestion", "ask_user_question", "mcp__rss_host__propose"],
  );
  assert.deepEqual(nativeToolInventory([{}, { tools: [] }]), []);
  for (const requests of [
    undefined,
    [],
    [{ tools: "unknown" }],
    [{ tools: [{}] }],
    [{ tools: [{ type: "namespace", name: "mcp", tools: [{}] }] }],
  ])
    assert.throws(() => nativeToolInventory(requests));
});
