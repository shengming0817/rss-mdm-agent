import { describe, it, expect, vi } from "vitest";
import { shallowRef } from "vue";
import {
  createOperationController,
  OperationDrafts,
} from "@rss-mdm-agent/ai-ui-bridge";
import type {
  ExecutionTaskDetails,
  BackendRequest,
  BackendTaskView,
} from "@rss-mdm-agent/execution-bindings/task-details";
import { operationSnapshot } from "@rss-mdm-agent/ai-contract/operations";
import type { AssistantController } from "../controller";
import fixtures from "../../../../../tests/assistant/execution-fixtures.json";
import { connectOperation } from "./host";
import { operationProjection } from "./projection";
function fixture() {
  const record: BackendRequest = {
    ...(structuredClone(fixtures.proposed) as BackendRequest),
    state: "draft",
    revision: 1,
    confirmation: null,
    failure: null,
    action: {
      kind: "policy",
      locator: { after: null },
      choice: {
        request: "operation-1",
        policyId: "policy-1",
        policyRevision: 1,
        policyVersion: "version-1",
        parameters: { name: "AI suggestion" },
      },
      directory: {
        policyId: "policy-1",
        policyRevision: 1,
        policyVersion: "version-1",
        subject: { kind: "device" },
        selfService: {
          published: true,
          allowAi: true,
          riskLevel: 2,
          presentation: {
            displayName: "Repair",
            description: "Trusted repair",
            category: "support",
            keywords: [],
            prerequisites: "device",
            sideEffects: "Changes configuration",
          },
        },
        task: {
          kind: "script",
          runAs: "system",
          resource: { id: "resource", version: "v1", variant: "default" },
          parameterSources: {
            name: { kind: "input" },
            path: { kind: "fixed", value: "/Applications" },
          },
          parameterSchema: {
            type: "object",
            properties: {
              name: { type: "string", maxLength: 32 },
              path: { type: "string" },
            },
            required: ["name", "path"],
          },
        },
      },
      access: { kind: "device" },
      runId: null,
      offer: null,
      expiresAtUnixMs: Date.now() + 60_000,
    },
  };
  const raw: BackendTaskView = { kind: "pending", value: record };
  let requests = 0,
    loseReply = false,
    rejection = "";
  const sent: unknown[] = [];
  const controller = {
    view: shallowRef({
      namespace: {
        sessionId: "session",
        principalId: "principal",
        authorityId: "authority",
        tenantId: "tenant",
      },
    }),
    executionDetails: async () => structuredClone(raw),
    submitOperation: async (input: any) => {
      if (rejection) throw { code: rejection };
      requests++;
      sent.push(input);
      if (record.action.kind !== "policy") throw Error("fixture");
      record.action.choice.parameters = input.parameters;
      record.revision++;
      record.state = "submitting";
      if (loseReply) throw Error("lost reply");
    },
    confirmPolicyPreparation: async () => {
      throw Error("duplicate confirmation");
    },
    cancelPreparation: async () => {
      record.state = "cancelled";
      record.revision++;
    },
  } as unknown as AssistantController;
  return {
    controller,
    record,
    raw,
    sent,
    requests: () => requests,
    loseReply: () => {
      loseReply = true;
    },
    reject: (code: string) => {
      rejection = code;
    },
  };
}

describe("production operation adapter", () => {
  it.each([undefined, 12, "   "])(
    "shows a stable policy title when presentation has no usable name: %s",
    async (displayName) => {
      const f = fixture();
      if (f.record.action.kind !== "policy") throw Error("fixture");
      f.record.action.directory.selfService.presentation =
        displayName === undefined ? {} : { displayName };
      const stop = new AbortController();
      const bridge = await connectOperation(
        f.controller,
        "delivery",
        stop.signal,
      );
      try {
        expect(bridge.snapshot.presentation.title).toBe("策略 policy-1");
        expect(bridge.snapshot.presentation.impact).toBe("未提供");
      } finally {
        stop.abort();
      }
    },
  );

  it("submits edited current values and preserves fixed parameters at the service boundary", async () => {
    const f = fixture(),
      stop = new AbortController();
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      stop.signal,
    );
    const card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      new OperationDrafts(),
    );
    expect(bridge.presentation.template).toBe("SettingsForm");
    card.edit("field0", "User edit");
    expect(f.requests()).toBe(0);
    expect(card.editable("field1")).toBe(false);
    await card.act("submit");
    expect(f.requests()).toBe(1);
    expect(f.sent[0]).toEqual({
      request: "operation-1",
      revision: 1,
      parameters: { name: "User edit" },
    });
    expect(card.state.snapshot.phase).toBe("waiting");
    card.dispose();
    stop.abort();
  });
  it("lost save receipt and renderer remount keep the original request until queried", async () => {
    const f = fixture(),
      stop = new AbortController(),
      drafts = new OperationDrafts();
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      stop.signal,
    );
    let card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    f.loseReply();
    card.edit("field0", "User edit");
    await card.act("submit");
    expect(card.state.unknown).toBe(true);
    card.fallback();
    card.dispose();
    card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    await card.act("submit");
    expect(f.requests()).toBe(1);
    await card.act("refresh");
    expect(card.can("submit")).toBe(false);
    expect(card.state.snapshot.scope.instance).toBe("operation-1");
    expect(card.state.snapshot.phase).toBe("waiting");
    card.dispose();
    stop.abort();
  });
  it("known native refusal preserves edits and does not become an ambiguous submission", async () => {
    const f = fixture(),
      stop = new AbortController();
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      stop.signal,
    );
    const card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      new OperationDrafts(),
    );
    card.edit("field0", "User edit");
    f.reject("operation_conflict");
    await card.act("submit");
    expect(card.state.unknown).toBe(false);
    expect(card.state.notice).toContain("版本");
    expect(card.state.draft.values.field0).toBe("User edit");
    expect(card.can("submit")).toBe(false);
    expect(f.requests()).toBe(0);
    card.dispose();
    stop.abort();
  });
  it("rejects a switched user and blocks expiry even without a server update", async () => {
    const f = fixture(),
      stop = new AbortController();
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      stop.signal,
    );
    const card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      new OperationDrafts(),
      () => bridge.snapshot.expiresAtMs!,
    );
    expect(card.can("submit")).toBe(false);
    card.dispose();
    f.controller.view.value!.namespace.principalId = "other";
    await expect(bridge.read()).rejects.toThrow("上下文");
    expect(f.requests()).toBe(0);
    stop.abort();
  });
  it("never labels execution-ended or unknown assessment as verified success", () => {
    const scope = {
      authority: "a",
      tenant: "t",
      principal: "p",
      session: "s",
      target: "d",
      instance: "i",
    };
    const raw: Extract<BackendTaskView, { kind: "execution" }> = {
      kind: "execution",
      value: structuredClone(fixtures.running) as ExecutionTaskDetails,
    };
    raw.value.status.phase = "executionEnded";
    expect(
      operationSnapshot(
        operationProjection(raw as BackendTaskView, scope, 1).snapshot,
      ).phase,
    ).toBe("execution_ended");
    raw.value.status.phase = "verified";
    raw.value.status.assessment = "unknown";
    expect(
      operationProjection(raw as BackendTaskView, scope, 2).snapshot.phase,
    ).toBe("unknown");
  });
  it("separates software effect target from enterprise admission and execution identity", async () => {
    for (const userTarget of [true, false]) {
      const f = fixture();
      if (f.record.action.kind !== "policy") throw Error("fixture");
      f.record.action.directory.subject = userTarget
        ? { kind: "device" }
        : {
            kind: "user",
            user: {
              tenantId: "enterprise-tenant",
              instanceId: "enterprise-instance",
              principalId: "enterprise-user",
            },
          };
      if (f.record.action.directory.task.kind !== "script")
        throw Error("script fixture");
      f.record.action.directory.task = {
        kind: "software",
        resource: f.record.action.directory.task.resource,
        runAs: "system",
        target: userTarget
          ? {
              kind: "user",
              identity: "S-1-5-21-OTHER",
              sessionId: "target-login2",
            }
          : { kind: "device" },
      };
      const bridge = await connectOperation(
        f.controller,
        "delivery",
        new AbortController().signal,
      );
      expect(bridge.snapshot.presentation.targetLabel).toContain(
        userTarget ? "S-1-5-21-OTHER / 登录会话 target-login2" : "本机设备",
      );
      expect(bridge.snapshot.presentation.targetLabel).not.toContain(
        "enterprise-user",
      );
      expect(bridge.snapshot.approval.enterprise).toContain(
        userTarget ? "设备准入主体" : "enterprise-user",
      );
      expect(bridge.snapshot.approval.elevation).toContain("系统执行身份");
    }
  });
  it("host query and remount recover uncertainty without replaying a failed save", async () => {
    const f = fixture(),
      drafts = new OperationDrafts();
    f.controller.submitOperation = async () => {
      throw Error("offline before write");
    };
    let bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    let card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    card.edit("field0", "User unsaved edit");
    await card.act("submit");
    expect(card.state.unknown).toBe(true);
    await bridge.read(); // The host button/poll path, not the renderer's refresh action.
    expect(card.state.unknown).toBe(false);
    expect(card.state.notice).toBe("");
    expect(card.editable("field0")).toBe(true);
    expect(card.can("submit")).toBe(true);
    await card.act("submit");
    card.dispose();
    bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    expect(card.state.unknown).toBe(false);
    expect(card.state.draft.values.field0).toBe("User unsaved edit");
    expect(f.requests()).toBe(0);
    card.dispose();
  });
  it("a read begun before a failed save cannot release the uncertainty barrier", async () => {
    const f = fixture();
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    const card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      new OperationDrafts(),
    );
    const read = f.controller.executionDetails;
    let finish!: (value: BackendTaskView) => void;
    f.controller.executionDetails = () =>
      new Promise((resolve) => {
        finish = resolve;
      });
    const stale = bridge.read().catch((error) => error);
    f.controller.submitOperation = async () => {
      throw Error("lost reply");
    };
    await card.act("submit");
    finish(structuredClone(f.raw));
    expect(await stale).toBeInstanceOf(Error);
    expect(card.state.unknown).toBe(true);
    expect(card.can("submit")).toBe(false);
    f.controller.executionDetails = read;
    await bridge.read();
    expect(card.state.unknown).toBe(false);
    expect(card.can("submit")).toBe(true);
    expect(f.requests()).toBe(0);
    card.dispose();
  });
  it("cancelled cards retain unsaved values across layout changes but reset incompatible fields", async () => {
    const f = fixture(),
      drafts = new OperationDrafts();
    let bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    let card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    card.edit("field0", "User unsaved edit");
    await card.act("cancel");
    card.dispose();
    bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    expect(bridge.presentation.template).toBe("OperationApproval");
    card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    expect(card.state.draft.values.field0).toBe("User unsaved edit");
    expect(card.state.draft.dirty.has("field0")).toBe(true);
    expect(card.can("submit")).toBe(false);
    expect(card.editable("field0")).toBe(false);
    expect(card.state.notice).not.toContain("版本已变化");
    card.dispose();
    const changed = structuredClone(bridge.presentation);
    changed.fields[0].maxLength = 16;
    card = createOperationController(
      changed,
      bridge.snapshot,
      bridge.host,
      drafts,
    );
    expect(card.state.draft.values.field0).toBe("AI suggestion");
    expect(card.state.draft.dirty.size).toBe(0);
    expect(card.state.notice).toContain("版本已变化");
    card.dispose();
  });
  it("remount reads stay newer after the system clock moves backwards", async () => {
    const f = fixture();
    const first = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    const clock = vi.spyOn(Date, "now").mockReturnValue(1);
    try {
      const next = await connectOperation(
        f.controller,
        "delivery",
        new AbortController().signal,
      );
      expect(next.snapshot.revision).toBeGreaterThan(first.snapshot.revision);
      expect((await next.read()).revision).toBeGreaterThan(
        next.snapshot.revision,
      );
    } finally {
      clock.mockRestore();
    }
  });
  it("still reads the authorized local request when backend reconciliation fails", async () => {
    const f = fixture();
    f.record.state = "submitting";
    f.controller.reconcilePolicyPreparation = async () => {
      throw Error("backend offline");
    };
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    f.record.state = "failed";
    f.record.failure = "revoked";
    const snapshot = await bridge.read();
    expect(snapshot.phase).toBe("failed");
    expect(snapshot.reason).toContain("revoked");
    expect(f.requests()).toBe(0);
  });
  it("default approval submits original array parameters without rewriting them as display text", async () => {
    const f = fixture();
    if (
      f.record.action.kind !== "policy" ||
      f.record.action.directory.task.kind !== "script"
    )
      throw Error("fixture");
    f.record.action.directory.task.parameterSchema = {
      type: "object",
      properties: {
        name: { type: "array", items: { type: "string" } },
        path: { type: "string" },
      },
      required: ["name", "path"],
    };
    f.record.action.choice.parameters.name = ["one"];
    const bridge = await connectOperation(
      f.controller,
      "delivery",
      new AbortController().signal,
    );
    expect(bridge.presentation.template).toBe("OperationApproval");
    const card = createOperationController(
      bridge.presentation,
      bridge.snapshot,
      bridge.host,
      new OperationDrafts(),
    );
    await card.act("submit");
    expect(f.requests()).toBe(1);
    expect(f.sent[0]).toEqual({
      request: "operation-1",
      revision: 1,
      parameters: { name: ["one"] },
    });
    card.dispose();
  });
  it("unsupported editable schemas use approval while secret inputs never enter the projection", () => {
    const f = fixture();
    if (
      f.record.action.kind !== "policy" ||
      f.record.action.directory.task.kind !== "script"
    )
      throw Error("fixture");
    const task = f.record.action.directory.task;
    task.parameterSchema = {
      type: "object",
      properties: { name: { type: "object" }, path: { type: "string" } },
    };
    f.record.action.choice.parameters.name = { nested: true };
    const scope = {
      authority: "a",
      tenant: "t",
      principal: "p",
      session: "s",
      target: "d",
      instance: "i",
    };
    expect(operationProjection(f.raw, scope, 1).presentation.template).toBe(
      "OperationApproval",
    );
    task.parameterSchema = {
      type: "object",
      properties: { name: { type: "string", writeOnly: true } },
    };
    f.record.action.choice.parameters.name = "do-not-display";
    const result = operationProjection(f.raw, scope, 2);
    expect(JSON.stringify(result.snapshot)).not.toContain("do-not-display");
    expect(result.snapshot.allowed).not.toContain("submit");
    task.parameterSchema = {
      type: "object",
      properties: {
        name: {
          type: "object",
          properties: { secret: { format: "password" } },
        },
      },
    };
    f.record.action.choice.parameters.name = {
      secret: "nested-do-not-display",
    };
    expect(
      JSON.stringify(operationProjection(f.raw, scope, 3).snapshot),
    ).not.toContain("nested-do-not-display");
    task.parameterSchema = { $ref: "#/unresolved" };
    expect(operationProjection(f.raw, scope, 4).snapshot.allowed).not.toContain(
      "submit",
    );
  });
});
