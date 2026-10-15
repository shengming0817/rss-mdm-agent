import { createOperationSurface } from "@rss-mdm-agent/ai-ui-bridge/surface";
import { describe, it, expect } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
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
import OperationCard from "./OperationCard.vue";
import { NodeView, operationContext } from "./NodeView";
import {
  Catalog,
  MessageProcessor,
  NodeResolver,
  getValue,
} from "@a2ui/web_core/v0_9";

const button = (wrapper: ReturnType<typeof mount>, name: string) =>
  wrapper.findAll("button").find((b) => b.text() === name)!;
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
  it("rejects duplicate fields, unsafe keys, arbitrary trees and invalid event kinds", () => {
    const f = operationFixture();
    expect(() =>
      operationPresentation({
        ...f.presentation,
        fields: [...f.presentation.fields, f.presentation.fields[0]],
      }),
    ).toThrow();
    expect(() =>
      operationPresentation({ ...f.presentation, html: "<script>" }),
    ).toThrow();
    expect(() =>
      operationPresentation({
        ...f.presentation,
        fields: [{ id: "constructor", label: "bad", component: "TextField" }],
      }),
    ).toThrow();
    expect(() => operationEvent({ kind: "executeShell" })).toThrow();
  });
});

describe("operation Vue templates", () => {
  it("recovers from an invalid projection by querying the last trusted scope", async () => {
    const f = operationFixture();
    let reads = 0;
    f.host.read = async () => {
      reads++;
      return f.snapshot;
    };
    const wrapper = mount(OperationCard, {
      props: {
        presentation: f.presentation,
        snapshot: f.snapshot,
        host: f.host,
        drafts: new OperationDrafts(),
      },
    });
    await wrapper.setProps({
      snapshot: { ...f.snapshot, phase: "invalid" } as never,
    });
    await flushPromises();
    expect(wrapper.text()).toContain("操作界面不可用");
    expect(
      button(wrapper, "查询原任务").attributes("disabled"),
    ).toBeUndefined();
    await button(wrapper, "查询原任务").trigger("click");
    await flushPromises();
    expect(reads).toBe(1);
    expect(wrapper.find('input[type="text"]').exists()).toBe(true);
    await wrapper.setProps({
      snapshot: { ...f.snapshot, phase: "invalid" } as never,
    });
    await wrapper.setProps({
      snapshot: { ...f.snapshot, revision: f.snapshot.revision + 1 },
    });
    await flushPromises();
    expect(wrapper.find('input[type="text"]').exists()).toBe(true);
    wrapper.unmount();
  });
  it("renders missing ordinary approval parameters and validates before dispatch", async () => {
    const f = operationFixture("OperationApproval");
    f.snapshot.current.name = "";
    f.snapshot.proposal = {};
    const wrapper = mount(OperationCard, {
      props: {
        presentation: f.presentation,
        snapshot: f.snapshot,
        host: f.host,
        drafts: new OperationDrafts(),
      },
    });
    await flushPromises();
    await button(wrapper, "确认操作").trigger("click");
    expect(f.events).toHaveLength(0);
    await wrapper.get('input[type="text"]').setValue("completed");
    await button(wrapper, "确认操作").trigger("click");
    await flushPromises();
    expect(f.events[0].values.name).toBe("completed");
    wrapper.unmount();
  });
  it("keeps required inputs usable in a GuidedFlow renderer fallback", async () => {
    const f = operationFixture("GuidedFlow");
    f.snapshot.current.name = "";
    f.snapshot.proposal = {};
    const c = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      new OperationDrafts(),
    );
    c.fallback();
    const surface = createOperationSurface(f.presentation, true);
    surface.update(c.state.draft.values);
    const wrapper = mount(NodeView, {
      props: { node: getValue(surface.root)! },
      global: { provide: { [operationContext as symbol]: c } },
    });
    await wrapper
      .get('[data-operation-field="name"] input')
      .setValue("fallback value");
    await button(wrapper, "确认操作").trigger("click");
    await flushPromises();
    expect(f.events[0].values.name).toBe("fallback value");
    wrapper.unmount();
    surface.dispose();
    c.dispose();
  });
  for (const layout of ["tabs", "disclosure"] as const)
    it(`reveals and focuses an invalid field inside ${layout}`, async () => {
      const f = operationFixture();
      f.presentation.layout = layout;
      f.presentation.fields.find((field) => field.id === "secret")!.required =
        true;
      const wrapper = mount(OperationCard, {
        attachTo: document.body,
        props: {
          presentation: f.presentation,
          snapshot: f.snapshot,
          host: f.host,
          drafts: new OperationDrafts(),
        },
      });
      if (layout === "disclosure")
        for (const details of wrapper.findAll("details"))
          details.element.removeAttribute("open");
      await button(wrapper, "保存").trigger("click");
      await flushPromises();
      const input = wrapper.get('[data-operation-field="secret"] input');
      expect(input.isVisible()).toBe(true);
      expect(document.activeElement).toBe(input.element);
      expect(f.events).toHaveLength(0);
      wrapper.unmount();
    });
  it("focuses step validation errors before advancing", async () => {
    const f = operationFixture("GuidedFlow");
    f.snapshot.current.enabled = true;
    const wrapper = mount(OperationCard, {
      attachTo: document.body,
      props: {
        presentation: f.presentation,
        snapshot: f.snapshot,
        host: f.host,
        drafts: new OperationDrafts(),
      },
    });
    await button(wrapper, "下一步").trigger("click");
    await flushPromises();
    const input = wrapper.get('[data-operation-field="address"] input');
    expect(input.isVisible()).toBe(true);
    expect(document.activeElement).toBe(input.element);
    wrapper.unmount();
  });
  for (const template of operationTemplates)
    it(`${template} renders actual controls and processes an action`, async () => {
      const f = operationFixture(template);
      const wrapper = mount(OperationCard, {
        props: {
          presentation: f.presentation,
          snapshot: f.snapshot,
          host: f.host,
          drafts: new OperationDrafts(),
        },
      });
      await flushPromises();
      expect(wrapper.text()).toContain(f.snapshot.targetLabel);
      expect(wrapper.text()).toContain("企业批准");
      if (template === "TaskExecution")
        await button(wrapper, "继续").trigger("click");
      else {
        if (template === "GuidedFlow")
          await button(wrapper, "下一步").trigger("click");
        if (template === "ResourceSelection" || template === "ObjectActions")
          await wrapper
            .get('input[aria-label="选择 测试对象 A"]')
            .setValue(true);
        await button(
          wrapper,
          template === "OperationApproval" ? "确认操作" : "保存",
        ).trigger("click");
      }
      await flushPromises();
      expect(f.events).toHaveLength(1);
      if (template !== "TaskExecution")
        expect(wrapper.text()).toContain("fixture 已核实");
      wrapper.unmount();
    });
  it("keyboard tabs, field errors, secret and path controls are usable", async () => {
    const f = operationFixture();
    const wrapper = mount(OperationCard, {
      attachTo: document.body,
      props: {
        presentation: f.presentation,
        snapshot: f.snapshot,
        host: f.host,
        drafts: new OperationDrafts(),
      },
    });
    await flushPromises();
    await wrapper.get('[role="switch"]').trigger("click");
    await button(wrapper, "保存").trigger("click");
    await flushPromises();
    expect(wrapper.find('[aria-invalid="true"]').exists()).toBe(true);
    (wrapper.findAll('[role="tab"]')[0].element as HTMLElement).focus();
    await wrapper
      .findAll('[role="tab"]')[0]
      .trigger("keydown", { key: "ArrowRight" });
    await flushPromises();
    expect(wrapper.findAll('[role="tab"]')[1].attributes("aria-selected")).toBe(
      "true",
    );
    await button(wrapper, "选择路径").trigger("click");
    await wrapper.get('input[type="password"]').setValue("secret-canary-2733");
    await flushPromises();
    expect(
      (wrapper.get('input[type="password"]').element as HTMLInputElement).value,
    ).toBe("");
    expect(wrapper.html()).not.toContain("secret-canary-2733");
    expect(f.secretCount()).toBe(1);
    wrapper.unmount();
    expect(f.secretCount()).toBe(0);
  });
});

// The same actual A2UI resolver/binding path used by one-shot question surfaces.
describe("question TextField variants", () => {
  for (const [variant, selector, value] of [
    ["longText", "textarea", "line one\nline two"],
    ["number", 'input[role="spinbutton"]', "42"],
    ["obscured", 'input[type="password"]', "masked input"],
  ])
    it(`preserves ${variant} rendering and writable binding`, async () => {
      const loaded = Catalog.fromSchema(operationCatalog);
      const catalog = new Catalog(
        loaded.id,
        [...loaded.components.values()],
        [],
      );
      const processor = new MessageProcessor([catalog], async () => {}, {
        version: "v0.9.1",
      });
      processor.processMessages([
        {
          version: "v0.9.1",
          createSurface: { surfaceId: "test", catalogId: catalog.id },
        },
        {
          version: "v0.9.1",
          updateComponents: {
            surfaceId: "test",
            components: [
              {
                id: "root",
                component: "TextField",
                label: "Question",
                variant,
                value: { path: "/answer" },
              },
            ],
          },
        },
      ]);
      const surface = processor.model.getSurface("test")!;
      surface.dataModel.set("/answer", "");
      const resolver = new NodeResolver(surface, catalog);
      const wrapper = mount(NodeView, {
        props: { node: getValue(resolver.rootNode)! },
      });
      await wrapper.get(selector).setValue(value);
      if (variant === "number") await wrapper.get(selector).trigger("blur");
      expect(surface.dataModel.get("/answer")).toBe(value);
      wrapper.unmount();
      resolver.dispose();
      processor.model.dispose();
    });
});
