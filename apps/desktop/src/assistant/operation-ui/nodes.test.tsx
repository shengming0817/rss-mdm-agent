import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { getValue } from "@a2ui/web_core/v0_9";
import {
  createOperationController,
  OperationDrafts,
} from "@rss-mdm-agent/ai-ui-bridge";
import { createOperationSurface } from "@rss-mdm-agent/ai-ui-bridge/surface";
import {
  operationFixture,
  operationTemplates,
} from "../../../../../tests/assistant/operation-fixtures";
import { NodeView } from "./nodes";
const releases: (() => void)[] = [];
afterEach(() => {
  for (const release of releases.splice(0)) release();
});
function mount(
  template: (typeof operationTemplates)[number] = "SettingsForm",
  drafts = new OperationDrafts(),
  f = operationFixture(template),
) {
  const controller = createOperationController(
      f.presentation,
      f.snapshot,
      f.host,
      drafts,
    ),
    surface = createOperationSurface(controller.presentation);
  surface.update({ ...controller.state.draft.values });
  const app = render(
    <NodeView
      node={getValue(surface.root)!}
      context={{ kind: "operation", controller }}
    />,
  );
  const dispose = () => {
    app.unmount();
    controller.dispose();
    surface.dispose();
  };
  releases.push(dispose);
  return { f, controller, surface, app, dispose, drafts };
}
it.each(operationTemplates)(
  "renders the official %s template with original facts and exact permitted controls",
  (template) => {
    const f = mount(template);
    expect(screen.getByText("登记操作 · 开发 fixture")).toBeTruthy();
    expect(screen.getByText("仅修改测试内存，不操作真实系统")).toBeTruthy();
    expect(screen.getByRole("button", { name: "查询原操作" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /全局|始终允许/ })).toBeNull();
    expect(f.f.events).toEqual([]);
  },
);
it("submits edits once through the trusted controller, preserves dirty values on projection and requires explicit rebase", async () => {
  const f = mount();
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), {
      target: { value: "用户核对后的值" },
    }),
  );
  await act(async () =>
    f.f.update({
      current: { ...f.f.snapshot.current, name: "外部更新" },
      baseline: "2",
    }),
  );
  expect(
    (
      screen.getByRole("textbox", {
        name: "名称",
      }) as HTMLInputElement
    ).value,
  ).toBe("用户核对后的值");
  expect(
    (
      screen.getByRole("button", {
        name: "确认本次具体动作",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "将草稿用于当前版本" })),
  );
  await act(async () => {
    const submit = screen.getByRole("button", { name: "确认本次具体动作" });
    fireEvent.click(submit);
    fireEvent.click(submit);
  });
  expect(f.f.events).toHaveLength(1);
  expect(f.f.events[0].values.name).toBe("用户核对后的值");
  expect(screen.getByText("已核实符合预期")).toBeTruthy();
});
it("maps all field types and captures a secret through its transient port without putting plaintext in a draft or event", async () => {
  const f = mount();
  await act(async () =>
    fireEvent.click(screen.getByRole("tab", { name: "高级" })),
  );
  expect(screen.getByRole("checkbox", { name: "启用通知" })).toBeTruthy();
  expect(screen.getByRole("radio", { name: "使用电池" })).toBeTruthy();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "选择路径" })),
  );
  expect(
    (screen.getByRole("textbox", { name: "配置路径" }) as HTMLInputElement)
      .value,
  ).toBe("/fixture/config");
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "执行时间" }), {
      target: { value: "2026-10-29T15:00:00Z" },
    }),
  );
  const secret = screen.getByLabelText("操作密码") as HTMLInputElement;
  await act(async () =>
    fireEvent.change(secret, { target: { value: "TRANSIENT-SECRET" } }),
  );
  await act(async () => fireEvent.blur(secret));
  expect(secret.value).toBe("");
  expect(f.f.secretCount()).toBe(1);
  expect(JSON.stringify(f.controller.state.draft.values)).not.toContain(
    "TRANSIENT-SECRET",
  );
  expect(
    (
      screen.getByRole("textbox", {
        name: "组织",
      }) as HTMLInputElement
    ).disabled,
  ).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("tab", { name: "常规" })),
  );
  expect(screen.getByRole("spinbutton", { name: "代理端口" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "网络范围" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "启用代理" })).toBeTruthy();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "确认本次具体动作" })),
  );
  expect(f.f.events).toHaveLength(1);
  expect(f.f.events[0].secretRefs.secret).toBeTruthy();
  expect(JSON.stringify(f.f.events)).not.toContain("TRANSIENT-SECRET");
  expect(f.f.secretCount()).toBe(0);
});
it("shows only current guided-step fields and validates before advancing", async () => {
  mount("GuidedFlow");
  expect(screen.getByRole("textbox", { name: "名称" })).toBeTruthy();
  expect(screen.queryByLabelText("操作密码")).toBeNull();
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), {
      target: { value: "" },
    }),
  );
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "下一步" })),
  );
  expect(screen.getByRole("alert").textContent).toBe("请填写此项");
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), {
      target: { value: "继续" },
    }),
  );
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "下一步" })),
  );
  expect(screen.queryByRole("textbox", { name: "名称" })).toBeNull();
  expect(screen.getByLabelText("操作密码")).toBeTruthy();
});
it("retains unknown state and original draft on remount, and only original read can restore permission to act", async () => {
  const fixture = operationFixture(),
    drafts = new OperationDrafts(),
    first = mount("SettingsForm", drafts, fixture);
  fixture.host.dispatch = async () => {
    throw Error("receipt lost");
  };
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), {
      target: { value: "原输入" },
    }),
  );
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "确认本次具体动作" })),
  );
  expect(first.controller.state.unknown).toBe(true);
  first.dispose();
  releases.splice(releases.indexOf(first.dispose), 1);
  const next = mount("SettingsForm", drafts, fixture);
  expect(
    (
      screen.getByRole("textbox", {
        name: "名称",
      }) as HTMLInputElement
    ).value,
  ).toBe("原输入");
  expect(
    (
      screen.getByRole("button", {
        name: "确认本次具体动作",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "查询原操作" })),
  );
  expect(next.controller.state.unknown).toBe(false);
});
it("keeps unavailable objects disabled and routes a pending answer through its exact allowed option", async () => {
  mount("ResourceSelection");
  const denied = screen.getByRole("button", { name: /测试对象 B/ });
  expect((denied as HTMLButtonElement).disabled).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: /测试对象 A/ })),
  );
  expect(
    screen
      .getByRole("button", { name: /测试对象 A/ })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  for (const release of releases.splice(0)) release();
  const f = mount("TaskExecution");
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: /继\s*续/ })),
  );
  expect(f.f.events[0]).toMatchObject({
    kind: "answer",
    answer: "yes",
    interactionId: "continue",
  });
  expect(
    within(f.app.container).queryByRole("button", { name: "确认本次具体动作" }),
  ).toBeNull();
});
