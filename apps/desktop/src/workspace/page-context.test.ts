import { expect, it } from "vitest";
import { pageContext } from "./page-context";
import { composePrompt } from "../assistant/resource-context";
it("attaches a bounded, immutable display projection without form or credential fields", () => {
  const input = {
    id: "app-1",
    title: "Notes",
    description: "Notes description",
    version: "2.1",
    status: "可用",
    source: "设计预览",
    apiKey: "SECRET",
    form: { password: "PASSWORD" },
  };
  const context = pageContext(
    "software-management",
    "软件管理",
    [{ id: "app-1", title: "Notes" }],
    Array.from({ length: 20 }, () => input),
    "anonymous",
  );
  expect(context.scope).toBe("page");
  expect(context.path).toEqual(["软件管理", "Notes"]);
  expect(context.text).not.toContain("SECRET");
  expect(context.text).not.toContain("PASSWORD");
  const data = JSON.parse(context.text.split("\n").slice(1).join("\n"));
  expect(data.resources).toHaveLength(12);
  input.title = "changed";
  expect(context.text).toContain("Notes");
  expect(Object.isFrozen(context)).toBe(true);
  expect(composePrompt({ text: "解释用途", context })).toBe(
    "解释用途\n\n" + context.text,
  );
});

it("binds the attachment to the native subject, resource revision and observation deadline", () => {
  const resource = {
    id: "r",
    title: "Resource",
    description: "Visible",
    source: "service",
    revision: "snapshot-1",
    collectedAt: 1000,
    validUntil: 2000,
    verified: true,
  };
  const first = pageContext(
    "software-management",
    "软件管理",
    [],
    [resource],
    "tenant-a",
    1000,
  );
  expect(composePrompt({ text: "why", context: first }, 1999)).toBeDefined();
  expect(composePrompt({ text: "why", context: first }, 2000)).toBeUndefined();
  expect(
    pageContext(
      "software-management",
      "软件管理",
      [],
      [resource],
      "tenant-b",
      1000,
    ).key,
  ).not.toBe(first.key);
  expect(
    pageContext(
      "software-management",
      "软件管理",
      [],
      [{ ...resource, revision: "snapshot-2" }],
      "tenant-a",
      1000,
    ).key,
  ).not.toBe(first.key);
  expect(
    pageContext(
      "software-management",
      "软件管理",
      [],
      [{ ...resource, verified: false }],
      "tenant-a",
      1000,
    ).stale,
  ).toBe(true);
});
