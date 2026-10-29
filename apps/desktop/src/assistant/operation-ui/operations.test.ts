const flushPromises = () =>
  new Promise<void>((resolve) => setTimeout(resolve, 0));
import { createOperationSurface } from "@rss-mdm-agent/ai-ui-bridge/surface";
import { describe, it, expect } from "vitest";

import {
  createOperationController,
  OperationDrafts,
} from "@rss-mdm-agent/ai-ui-bridge";
import {
  operationPresentation,
  operationEvent,
  operationCatalog,
} from "@rss-mdm-agent/ai-contract/operations";
import {
  operationFixture,
  operationTemplates,
} from "../../../../../tests/assistant/operation-fixtures";

import {
  Catalog,
  MessageProcessor,
  NodeResolver,
  getValue,
} from "@a2ui/web_core/v0_9";

describe("operation controllers", () => {
  it("preserves edits across updates/remounts and forces reconciliation after baseline change", async () => {
    const f = operationFixture(),
      drafts = new OperationDrafts();
    let c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      drafts,
    );
    c.edit("name", "用户修改");
    f.update({ proposal: { name: "后续建议" } });
    expect(c.state.draft.values.name).toBe("用户修改");
    c.dispose();
    c = createOperationController(f.presentation, f.snapshot, f.host, drafts);
    expect(c.state.draft.values.name).toBe("用户修改");
    f.update({ baseline: "2", current: { name: "外部修改" } });
    expect(c.can("submit")).toBe(false);
    expect(c.state.draft.values.name).toBe("用户修改");
    c.rebase();
    await c.act("submit");
    expect(f.events[0].values.name).toBe("用户修改");
    c.dispose();
  });
  it("adopts normalized successful receipts and preserves failed edits", async () => {
    const f = operationFixture();
    let phase: "ready" | "failed" = "ready";
    f.host.dispatch = async (event) => {
      f.events.push(event);
      return {
        ...f.snapshot,
        revision: f.snapshot.revision + 1,
        baseline: "new",
        phase,
        current: { ...f.snapshot.current, name: "HOST NORMALIZED" },
        proposal: {},
      };
    };
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    c.edit("name", "user raw");
    await c.act("submit");
    expect(c.state.draft.values.name).toBe("HOST NORMALIZED");
    expect(c.state.draft.dirty.size).toBe(0);
    await c.act("submit");
    expect(f.events[1].values.name).toBe("HOST NORMALIZED");
    c.edit("name", "keep on failure");
    phase = "failed";
    await c.act("submit");
    expect(c.state.draft.values.name).toBe("keep on failure");
    expect(c.state.draft.dirty.has("name")).toBe(true);
    c.dispose();
  });
  it("removes withdrawn suggestions without discarding user edits", () => {
    const f = operationFixture();
    f.snapshot.proposal = { name: "withdrawn", address: "user suggestion" };
    delete f.snapshot.current.name;
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    c.edit("address", "my address");
    f.update({ proposal: {} });
    expect(c.state.draft.values.name).toBeUndefined();
    expect(c.state.draft.values.address).toBe("my address");
    c.dispose();
  });
  it("allows trusted task controls despite a stale incomplete form", () => {
    const f = operationFixture("TaskExecution");
    f.presentation.fields.find((field) => field.id === "secret")!.required =
      true;
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    f.update({ baseline: "changed" });
    expect(c.state.stale).toBe(true);
    expect(c.can("cancel")).toBe(true);
    expect(c.can("answer")).toBe(true);
    c.dispose();
  });
  it("validates cross fields and distinguishes draft from immediate switch", async () => {
    const f = operationFixture(),
      c = createOperationController(
        f.presentation,
        f.snapshot,
        f.host,
        new OperationDrafts(),
      );
    c.edit("enabled", true);
    await c.act("submit");
    expect(f.events).toHaveLength(0);
    expect(c.state.errors.address).toBeTruthy();
    c.edit("address", "localhost");
    await c.act("submit");
    expect(f.events).toHaveLength(1);
    c.dispose();
    const second = operationFixture();
    second.presentation.fields.find((v) => v.id === "enabled")!.submitMode =
      "immediate";
    const immediate = createOperationController(
      second.presentation,
      second.snapshot,
      second.host,
      new OperationDrafts(),
    );
    immediate.edit("enabled", false);
    await flushPromises();
    expect(second.events).toHaveLength(1);
    immediate.dispose();
  });
  it("does not repeat an unknown execution and queries the original instance", async () => {
    const f = operationFixture();
    f.loseReceipt();
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    await c.act("submit");
    expect(c.state.unknown).toBe(true);
    await c.act("submit");
    expect(f.events).toHaveLength(1);
    await c.act("refresh");
    expect(c.state.unknown).toBe(false);
    expect(c.state.snapshot.phase).toBe("succeeded");
    c.dispose();
  });
  it("keeps pending and unknown execution barriers across remounts", async () => {
    const f = operationFixture(),
      drafts = new OperationDrafts();
    let calls = 0;
    let reject!: (error: Error) => void;
    f.host.dispatch = async () => {
      calls++;
      return new Promise((_, fail) => {
        reject = fail;
      });
    };
    let c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      drafts,
    );
    const pending = c.act("submit");
    c.dispose();
    c = createOperationController(f.presentation, f.snapshot, f.host, drafts);
    expect(c.state.busy).toBe(true);
    await c.act("submit");
    expect(calls).toBe(1);
    reject(new Error("receipt lost"));
    await pending;
    expect(c.state.unknown).toBe(true);
    c.dispose();
    c = createOperationController(f.presentation, f.snapshot, f.host, drafts);
    await c.act("submit");
    expect(calls).toBe(1);
    f.update({ revision: f.snapshot.revision + 2 });
    f.host.read = async () => ({ ...f.snapshot, revision: 0 });
    await c.act("refresh");
    expect(c.state.unknown).toBe(true);
    f.host.read = async () => f.snapshot;
    await c.act("refresh");
    expect(c.state.unknown).toBe(false);
    c.dispose();
  });
  it("blocks submission during secret replacement and revokes late capture on cancel", async () => {
    const f = operationFixture();
    const revoked: string[] = [];
    let finish!: (ref: string) => void;
    f.host.revokeSecret = (ref) => {
      revoked.push(ref);
    };
    f.host.captureSecret = async () => "old-ref";
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    await c.secret("secret", "first");
    f.host.captureSecret = async () =>
      new Promise((resolve) => {
        finish = resolve;
      });
    const capture = c.secret("secret", "replacement");
    expect(revoked).toContain("old-ref");
    expect(c.can("submit")).toBe(false);
    await c.act("submit");
    expect(f.events).toHaveLength(0);
    await c.act("cancel");
    expect(f.events[0].secretRefs).toEqual({});
    expect(f.events[0].values).toEqual({});
    finish("late-ref");
    await capture;
    expect(revoked).toContain("late-ref");
    expect(c.state.secrets).toEqual({});
    c.dispose();
  });
  it("isolates secret inputs, revokes them on unmount and never grants path access", async () => {
    const f = operationFixture(),
      drafts = new OperationDrafts();
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      drafts,
    );
    await c.secret("secret", "secret-canary-2733");
    expect(f.secretCount()).toBe(1);
    expect(JSON.stringify(c.state)).not.toContain("secret-canary-2733");
    await c.choosePath("path");
    expect(c.state.draft.values.path).toBe("/fixture/config");
    expect(f.events).toHaveLength(0);
    c.dispose();
    expect(f.secretCount()).toBe(0);
    const restored = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      drafts,
    );
    expect(restored.state.secrets).toEqual({});
    restored.dispose();
  });
  it("falls back without converting unsupported presentation versions", async () => {
    const f = operationFixture();
    const c = createOperationController(
      { ...f.presentation, version: 99 },
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    expect(c.presentation.template).toBe("OperationApproval");
    await c.act("submit");
    expect(f.events[0].instance).toBe(f.snapshot.scope.instance);
    c.dispose();
  });
  it("rejects duplicate fields, unsafe keys and invalid actions while retaining inert display extensions", () => {
    const f = operationFixture();
    expect(() =>
      operationPresentation({
        ...f.presentation,
        fields: [...f.presentation.fields, f.presentation.fields[0]],
      }),
    ).toThrow();
    const extended = operationPresentation({
      ...f.presentation,
      html: "<script>",
    });
    expect(extended.html).toBe("<script>");
    const surface = createOperationSurface(extended);
    expect(getValue(surface.root)?.type).toBe("OperationSurface");
    surface.dispose();
    expect(() =>
      operationPresentation({
        ...f.presentation,
        fields: [{ id: "constructor", label: "bad", component: "TextField" }],
      }),
    ).toThrow();
    expect(() => operationEvent({ kind: "executeShell" })).toThrow();
  });
});

it("unknown presentation survives while secret handling belongs to the trusted snapshot", () => {
  const f = operationFixture();
  f.presentation.brandColor = { nested: ["blue"] };
  f.presentation.fields.find((field) => field.id === "secret")!.component =
    "TextField";
  const c = createOperationController(
    f.presentation,
    f.snapshot,
    f.host,
    new OperationDrafts(),
  );
  expect(c.presentation.brandColor).toEqual({ nested: ["blue"] });
  expect(c.isSecret("secret")).toBe(true);
  c.edit("secret", "must-not-persist");
  expect(c.state.draft.values.secret).toBeUndefined();
  c.dispose();
});
