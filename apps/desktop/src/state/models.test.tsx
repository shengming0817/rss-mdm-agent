import { act, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useSnapshot } from "valtio";
import { snapshot } from "valtio/vanilla";
import { createAssistant } from "../assistant/controller";
import { createSoftware } from "../software/controller";
import type { SettingsResponse } from "../self-service/types";
import { createSystemSettings } from "../system-settings/controller";
import { createTaskCenter } from "../task-center/controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import { taskDetail, taskPort } from "../../../../tests/self-service/support";
import {
  createOperationController,
  OperationDrafts,
} from "@rss-mdm-agent/ai-ui-bridge";
import type { OperationSnapshot } from "@rss-mdm-agent/ai-contract/operations";
import { operationFixture } from "../../../../tests/assistant/operation-fixtures";
afterEach(() => vi.useRealTimers());
it("React reads immutable derived snapshots after asynchronous inventory and task Map updates", async () => {
  const local = fixtureLocalPorts(),
    software = createSoftware(local.data, local.software),
    tasks = createTaskCenter(taskPort(), null);
  function Probe() {
    const inventory = useSnapshot(software.state),
      task = useSnapshot(tasks.state);
    return (
      <output>
        {software.summary(inventory).installed}:{tasks.recentItems(task).length}
      </output>
    );
  }
  const app = render(<Probe />),
    before = snapshot(software.state);
  await act(async () => {
    await software.discover();
    await tasks.openTask(taskDetail().reference);
  });
  expect(screen.getByRole("status").textContent).toBe("4:0");
  expect(before.sources).toHaveLength(0);
  expect(software.summary(snapshot(software.state)).installed).toBe(4);
  expect(snapshot(tasks.state).byRef.size).toBe(1);
  expect(tasks.state.selected?.facts).toEqual(taskDetail().facts);
  app.unmount();
  software.dispose();
  tasks.dispose();
});
it("React observes session draft Map, sending Set and time-derived state without mutating previous snapshots", async () => {
  vi.useFakeTimers();
  let time = 100;
  const assistant = createAssistant(
    undefined,
    () => "id",
    () => time,
  );
  function Probe() {
    const state = useSnapshot(assistant.state);
    return (
      <output>
        {assistant.draft.get(state)}:{state.sending.size}:
        {assistant.compositionError(state)}
      </output>
    );
  }
  const app = render(<Probe />),
    before = snapshot(assistant.state);
  await act(async () => {
    assistant.draft.set("first");
    assistant.stageContext({
      scope: "page",
      key: "resource",
      path: [],
      text: "context",
      stale: false,
      ownerKey: "anonymous",
      expiresAt: 500,
    });
    assistant.state.sending.add("command");
  });
  expect(screen.getByRole("status").textContent).toBe("first:1:");
  time = 600;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1000);
  });
  expect(screen.getByRole("status").textContent).toContain("资源信息已变更");
  expect(before.drafts.size).toBe(0);
  expect(before.sending.size).toBe(0);
  expect(before.now).toBe(100);
  app.unmount();
  assistant.dispose();
  expect(vi.getTimerCount()).toBe(0);
});
it("workspace draft retirement releases observers, deadlines and both current and late secret handles", async () => {
  vi.useFakeTimers();
  const fixture = operationFixture(),
    drafts = new OperationDrafts();
  fixture.snapshot.expiresAtMs = Date.now() + 10000;
  const stop = vi.fn(),
    observe = fixture.host.observe;
  fixture.host.observe = (scope, listener) => {
    const release = observe(scope, listener);
    return () => {
      stop();
      release();
    };
  };
  const revoke = vi.fn(fixture.host.revokeSecret);
  fixture.host.revokeSecret = revoke;
  const controller = createOperationController(
    fixture.presentation,
    fixture.snapshot,
    fixture.host,
    drafts,
  );
  function Probe() {
    const state = useSnapshot(controller.state);
    return (
      <output>
        {String(state.draft.values.name)}:{state.draft.dirty.size}:
        {state.snapshot.revision}:{String(state.busy)}
      </output>
    );
  }
  const app = render(<Probe />);
  await act(async () => {
    controller.edit("name", "PRIVATE");
  });
  expect(screen.getByRole("status").textContent).toContain("PRIVATE:1:1:false");
  await act(async () => {
    fixture.update({ current: { ...fixture.snapshot.current, name: "new" } });
  });
  expect(screen.getByRole("status").textContent).toContain("PRIVATE:1:2:false");
  await controller.secret("secret", "clear text");
  const handle = controller.state.secrets.secret;
  let release!: (reference: string) => void;
  fixture.host.captureSecret = () =>
    new Promise((resolve) => {
      release = resolve;
    });
  const late = controller.secret("secret", "second");
  let complete!: (value: OperationSnapshot) => void;
  fixture.host.read = () =>
    new Promise((resolve) => {
      complete = resolve;
    });
  const request = controller.act("refresh");
  app.unmount();
  drafts.clear();
  expect(stop).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
  expect(revoke).toHaveBeenCalledWith(handle);
  expect(controller.state.draft.values).toEqual({});
  const revision = controller.state.snapshot.revision;
  release("late-secret");
  complete({ ...fixture.snapshot, revision: 99 });
  await Promise.all([late, request]);
  expect(revoke).toHaveBeenCalledWith("late-secret");
  expect(controller.state.snapshot.revision).toBe(revision);
  expect(controller.can("submit")).toBe(false);
  expect(controller.state.busy).toBe(false);
});
it("disposing system reads releases an awaited poll without cancelling the backend request", async () => {
  vi.useFakeTimers();
  const local = fixtureLocalPorts();
  const port = vi.fn(
    async (): Promise<SettingsResponse> => ({
      writeAllowed: false,
      finished: false,
      request: "request",
      digest: null,
      effect: "pending",
      snapshot: {
        section: "power",
        revision: "a".repeat(64),
        collectedAtUnixMs: 1000,
        source: "fixture",
        targets: [],
      },
      keepBeforeUnixMs: null,
      reason: null,
    }),
  );
  const model = createSystemSettings(port, local.data);
  const pending = model.load("power");
  await vi.advanceTimersByTimeAsync(0);
  model.dispose();
  await pending;
  expect(vi.getTimerCount()).toBe(0);
  expect(model.state.busy.size).toBe(0);
});
