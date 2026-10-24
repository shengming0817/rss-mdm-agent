import { it, expect } from "vitest";
import { composePrompt } from "./resource-context";
import { pageContext } from "../workspace/page-context";
it("uses the same frozen page projection for preview and sending", () => {
  const c = pageContext(
    "software-management",
    "软件管理",
    [],
    [
      {
        id: "r",
        title: "办公套件",
        description: "展示信息",
        source: "授权目录",
        revision: "v1",
      },
    ],
    "owner",
    100,
  );
  expect(composePrompt({ text: " 解释版本 ", context: c }, 101)).toBe(
    "解释版本\n\n" + c.text,
  );
  expect(c.scope).toBe("page");
});
it("rejects empty, expired, stale and over-budget page compositions", () => {
  const c = pageContext("home", "主页", [], [], "owner", 100);
  expect(composePrompt({ text: "", context: c }, 101)).toBeUndefined();
  expect(
    composePrompt({ text: "question", context: c }, c.expiresAt),
  ).toBeUndefined();
  expect(
    composePrompt({ text: "question", context: { ...c, stale: true } }, 101),
  ).toBeUndefined();
  expect(composePrompt({ text: "x".repeat(65537) }, 101)).toBeUndefined();
  expect(composePrompt({ text: "question" }, 101)).toBe("question");
});
