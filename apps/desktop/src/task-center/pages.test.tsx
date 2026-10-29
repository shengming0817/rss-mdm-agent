import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { App } from "../App";
import { DesktopProvider, useDesktop } from "../state/react";
import type { Desktop } from "../state/desktop";
import { shellFixture } from "../../../../tests/desktop/ui-support";
import { configurePageFixture } from "../../../../tests/assistant/page-fixture";
import { taskDetail } from "../../../../tests/self-service/support";
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => false,
  invoke: vi.fn(),
}));
async function mount(scene = "tasks") {
  const fixture = shellFixture(),
    pages = configurePageFixture(fixture.input, scene),
    taskPort = fixture.input.environment!.local.tasks!;
  const list = vi.spyOn(taskPort, "list"),
    control = vi.spyOn(taskPort, "control");
  let desktop!: Desktop;
  function Observe() {
    desktop = useDesktop();
    return null;
  }
  const app = render(
    <DesktopProvider input={fixture.input}>
      <App />
      <Observe />
    </DesktopProvider>,
  );
  await waitFor(() => expect(desktop?.state.workspace).toBeTruthy());
  const workspace = desktop.state.workspace!;
  await screen.findByRole("button", { name: "切换侧栏布局" });
  fireEvent.click(
    within(
      app.container.querySelector(".rss-hidden-primary") as HTMLElement,
    ).getByRole("button", { name: "任务中心" }),
  );
  const page = () =>
    within(
      app.container.querySelector(
        '.rss-page-viewport[aria-label="任务中心"]',
      ) as HTMLElement,
    );
  await page().findByRole("table");
  const open = async (request: string) => {
    const row = page().getByText(request, { exact: true }).closest("tr")!;
    fireEvent.click(within(row).getByRole("button", { name: "查看详情" }));
    await waitFor(() => expect(workspace.tasks.state.selected).toBeTruthy());
    await waitFor(() =>
      expect(document.querySelector(".rss-task-detail h3")).not.toBeNull(),
    );
    return within(
      document
        .querySelector(".rss-task-detail")!
        .closest('[role="dialog"]') as HTMLElement,
    );
  };
  return {
    fixture,
    pages,
    taskPort,
    list,
    control,
    app,
    workspace,
    desktop,
    page,
    open,
  };
}
it("uses the original task owner for three views, real nodes and assessment-aware history", async () => {
  const f = await mount();
  expect(f.list).toHaveBeenCalledWith(
    expect.objectContaining({ view: "active", source: null }),
  );
  const detail = await f.open("fixture-update");
  expect(detail.getByText("执行已结束，等待效果验证")).toBeTruthy();
  expect(
    f.app.container.ownerDocument.querySelector(".ant-timeline"),
  ).toBeTruthy();
  expect(detail.queryByText("已核实符合预期")).toBeNull();
  fireEvent.click(detail.getByRole("button", { name: "查询原请求" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter(
        (event) =>
          event.operation === "task_detail" &&
          event.request === "fixture-update",
      ).length,
    ).toBeGreaterThanOrEqual(2),
  );
  expect(f.control).not.toHaveBeenCalled();
  act(() => {
    f.workspace.tasks.closeTask();
    f.workspace.navigation.select("tasks", "management");
  });
  await f.page().findByText("fixture-policy-activity", { exact: true });
  expect(f.list).toHaveBeenLastCalledWith(
    expect.objectContaining({ view: "management" }),
  );
  act(() => {
    f.workspace.navigation.select("tasks", "history");
  });
  await f.page().findByText("已核实未达预期");
  expect(f.page().getByText("已核实符合预期")).toBeTruthy();
  expect(f.page().getByText(/摘要仅统计本页已读取/)).toBeTruthy();
  f.app.unmount();
});
it("recovers the same cancellation receipt and keeps running until independent termination", async () => {
  const f = await mount("tasks-control-unknown"),
    detail = await f.open("fixture-install");
  fireEvent.click(detail.getByRole("button", { name: "请求取消" }));
  // rc-util deliberately reuses test-id in NODE_ENV=test; the real browser checks dialog names.
  const confirmation = within(
    document.querySelector('.ant-modal[role="dialog"]') as HTMLElement,
  );
  expect(confirmation.getByText("请求取消？")).toBeTruthy();
  const confirm = confirmation.getByRole("button", { name: "请求取消" });
  fireEvent.click(confirm);
  fireEvent.click(confirm);
  await waitFor(() =>
    expect(f.workspace.tasks.state.pendingControl).toBeTruthy(),
  );
  expect(f.control).toHaveBeenCalledTimes(1);
  const original = f.workspace.tasks.state.pendingControl!;
  expect(original.reference).toEqual({
    kind: "request",
    request: "fixture-install",
  });
  expect(f.workspace.tasks.state.selected?.stage).toBe("running");
  await detail.findByText("已请求取消");
  fireEvent.click(detail.getByRole("button", { name: "查询原控制请求" }));
  await waitFor(() =>
    expect(f.workspace.tasks.state.pendingControl).toBeNull(),
  );
  expect(
    f.pages.events.find((event) => event.operation === "task_control_status")
      ?.controlId,
  ).toBe(original.id);
  expect(f.control).toHaveBeenCalledTimes(1);
  expect(f.workspace.tasks.state.selected?.stage).toBe("running");
  f.pages.advanceTask("fixture-install", "cancelled", "noEffect");
  fireEvent.click(detail.getByRole("button", { name: "查询原请求" }));
  await detail.findByText("可信证据确认取消且无效果");
  expect(detail.queryByRole("button", { name: "请求取消" })).toBeNull();
  f.app.unmount();
});
it("revoked detail access removes cached facts and controls, and a late response cannot enter a new workspace", async () => {
  const f = await mount(),
    detail = await f.open("fixture-ai-unknown");
  const read = vi.spyOn(f.taskPort, "detail");
  read.mockRejectedValueOnce({ code: "task_denied" });
  fireEvent.click(detail.getByRole("button", { name: "查询原请求" }));
  await waitFor(() => expect(f.workspace.tasks.state.selected).toBeNull());
  expect(
    f.workspace.tasks.state.byRef.has(
      JSON.stringify({ kind: "request", request: "fixture-ai-unknown" }),
    ),
  ).toBe(false);
  let finish!: (task: ReturnType<typeof taskDetail>) => void;
  read.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  let pending!: Promise<unknown>;
  act(() => {
    pending = f.workspace.tasks.openTask({
      kind: "request",
      request: "old-private-request",
    });
  });
  await act(async () => {
    await f.desktop.guest();
  });
  await act(async () => {
    finish(taskDetail("old-private-request"));
    await pending;
  });
  expect(f.workspace.tasks.state.selected).toBeNull();
  expect(f.desktop.state.workspace?.key).not.toBe(f.workspace.key);
  expect(f.app.container.textContent).not.toContain("old-private-request");
  expect(f.control).not.toHaveBeenCalled();
  f.app.unmount();
});
it.each(["企业准入拒绝", "风险尚未确定", "当前 OS 权限不允许", "原准备已过期"])(
  "never exposes confirmation or retry for a service denial: %s",
  async (reason) => {
    const f = await mount();
    const original = f.pages.details.get("fixture-install")!;
    original.stage = "admissionDenied";
    original.allowedActions = [];
    original.controlReason = reason;
    if (original.facts.kind === "execution")
      original.facts.value.status.phase = "admissionDenied";
    const detail = await f.open("fixture-install");
    expect(detail.getByText(reason)).toBeTruthy();
    expect(detail.queryByRole("button", { name: "请求取消" })).toBeNull();
    expect(
      detail.queryByRole("button", { name: "重试已知失败的任务" }),
    ).toBeNull();
    expect(detail.queryByText("已核实符合预期")).toBeNull();
    expect(f.control).not.toHaveBeenCalled();
    f.app.unmount();
  },
);
it.each(["install", "update", "uninstall"] as const)(
  "keeps %s submission, original task detail and independent verification on one request",
  async (intent) => {
    const f = await mount();
    let request: string;
    if (intent === "uninstall") {
      await act(async () => {
        await f.workspace.software.discover();
        const item = f.workspace.software.installed()[1];
        const prepared = await f.workspace.software.prepare(item, "uninstall");
        request = prepared!.request;
        await f.workspace.software.submit(prepared!);
      });
    } else {
      const item = f.pages.published[intent === "install" ? 0 : 1];
      await act(async () => {
        await f.workspace.policies.select(item);
        await f.workspace.policies.submit({});
        request = f.workspace.policies.state.submission!.choice.request;
      });
    }
    await act(async () => {
      await f.workspace.tasks.openTask({ kind: "request", request: request! });
    });
    await waitFor(() =>
      expect(document.querySelector(".rss-task-detail h3")).not.toBeNull(),
    );
    const detail = within(
      document
        .querySelector(".rss-task-detail")!
        .closest('[role="dialog"]') as HTMLElement,
    );
    expect(f.workspace.tasks.state.selected?.stage).toBe("accepted");
    if (intent === "update") {
      const selected = f.workspace.tasks.state.selected!;
      expect(selected.softwareState).toEqual({
        kind: "present",
        version: "1.0",
      });
      const record = f.workspace.policies.state.preparation!;
      expect(record.action.kind).toBe("policy");
      if (
        record.action.kind !== "policy" ||
        record.action.task.kind !== "software" ||
        selected.facts.kind !== "execution"
      )
        throw Error("software preparation required");
      expect(selected.facts.value.action.operation.resource).toEqual({
        id: record.action.task.resource.id,
        revision: record.action.task.resource.version,
      });
      expect(selected.facts.value.action.execution).toMatchObject({
        kind: "softwareProgram",
        intent: "install",
        steps: [{ package: record.action.task.resource.id, version: "2.0" }],
      });
      expect(record.action.offer?.presentation.summary).toMatchObject({
        kind: "software",
        intent: "install",
        steps: [{ package: record.action.task.resource.id, version: "2.0" }],
      });
    }
    for (const phase of ["running", "executionEnded"] as const) {
      f.pages.advanceTask(request!, phase);
      fireEvent.click(detail.getByRole("button", { name: "查询原请求" }));
      await waitFor(() =>
        expect(f.workspace.tasks.state.selected?.stage).toBe(phase),
      );
      expect(detail.queryByText("已核实符合预期")).toBeNull();
    }
    if (intent === "update")
      expect(f.workspace.tasks.state.selected?.softwareState).toEqual({
        kind: "present",
        version: "1.0",
      });
    f.pages.advanceTask(
      request!,
      "verified",
      "satisfied",
      intent === "uninstall"
        ? { kind: "absent" }
        : { kind: "present", version: intent === "update" ? "2.0" : "1.0" },
    );
    fireEvent.click(detail.getByRole("button", { name: "查询原请求" }));
    await detail.findByText("已核实符合预期");
    expect(f.workspace.tasks.state.selected?.softwareState).toEqual(
      intent === "uninstall"
        ? { kind: "absent" }
        : { kind: "present", version: intent === "update" ? "2.0" : "1.0" },
    );
    const writes = f.pages.events.filter((event) =>
      ["policy_invoke", "software_submit"].includes(event.operation),
    );
    expect(writes).toHaveLength(1);
    expect(writes[0].request).toBe(request!);
    expect(f.control).not.toHaveBeenCalled();
    f.app.unmount();
  },
);
