import { describe, it, expect } from "vitest";
import { resourceOffer } from "../../../../tests/self-service/support";
import {
  resourceContext,
  contextCurrent,
  composePrompt,
} from "./resource-context";

describe("resource context projection", () => {
  it("uses existing backend display information, exact versions and honest missing fields without task/account data", () => {
    const item = resourceOffer();
    Object.assign(item, {
      description: "UNAPPROVED_FIELD",
      inputs: { secret: "SECRET_INPUT" },
      deviceList: ["SECRET_DEVICE"],
    });
    const c = resourceContext(item, "live");
    expect(c.path).toEqual(["软件中心", "未提供分类", "办公套件", "软件详情"]);
    const data = JSON.parse(c.text.split("\n").slice(1).join("\n"));
    expect(data.resourceVersions).toEqual([
      { package: "办公套件", version: "1.0" },
    ]);
    expect(data.sourceRevision).toBe(item.revision);
    expect(data.catalogVersion).toBe("未提供");
    expect(data.display).toEqual({
      visibility: "allowed",
      requestability: "unknown",
      executability: "unknown",
    });
    for (const excluded of [
      item.task,
      item.attempt,
      item.request,
      "system",
      "SECRET_INPUT",
      "SECRET_DEVICE",
      "UNAPPROVED_FIELD",
      "userInitiated",
      "expiresAt",
    ])
      expect(c.text).not.toContain(excluded);
    expect(composePrompt({ text: " 解释版本 ", context: c })).toBe(
      "解释版本\n\n" + c.text,
    );
  });
  it("freezes the projection and invalidates revised, replaced or removed resources", () => {
    const item = resourceOffer();
    const c = resourceContext(item, "s1");
    const original = c.text;
    expect(contextCurrent(c, [item], "s1")).toBe(true);
    item.revision = "b".repeat(64);
    expect(c.text).toBe(original);
    expect(contextCurrent(c, [item], "s1")).toBe(false);
    expect(contextCurrent(c, [], "s1")).toBe(false);
    expect(contextCurrent(c, [resourceOffer()], "live")).toBe(false);
    expect(
      contextCurrent(c, [{ ...resourceOffer(), task: "replacement" }], "s1"),
    ).toBe(false);
  });
  it("projects scripts without invention and blocks empty, stale or over-budget submissions", () => {
    const item = {
      ...resourceOffer(),
      title: "诊断脚本",
      summary: { kind: "script" as const, identity: "system" as const },
    };
    const c = resourceContext(item, "s1");
    expect(c.path).toEqual(["工具中心", "未提供分类", "诊断脚本", "脚本详情"]);
    expect(c.text).toContain('"resourceVersions": []');
    expect(c.text).not.toContain("system");
    expect(composePrompt({ text: "", context: c })).toBeUndefined();
    expect(composePrompt({ text: "x".repeat(65537) })).toBeUndefined();
    expect(
      composePrompt({ text: "question", context: { ...c, stale: true } }),
    ).toBeUndefined();
  });
});
