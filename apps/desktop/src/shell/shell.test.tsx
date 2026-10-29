import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { App } from "../App";
import { DesktopProvider, useDesktop } from "../state/react";
import type { Desktop } from "../state/desktop";
import { shellFixture } from "../../../../tests/desktop/ui-support";
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => false,
  invoke: vi.fn(),
}));
afterEach(() => vi.useRealTimers());
async function fixture(
  configure?: (f: ReturnType<typeof shellFixture>) => void,
) {
  const f = shellFixture();
  configure?.(f);
  let desktop!: Desktop;
  function Observe() {
    desktop = useDesktop();
    return null;
  }
  const app = render(
    <DesktopProvider input={f.input}>
      <App />
      <Observe />
    </DesktopProvider>,
  );
  await screen.findByRole("button", { name: "切换侧栏布局" });
  await within(
    app.container.querySelector(
      '.rss-page-viewport[aria-label="主页"]',
    ) as HTMLElement,
  ).findByText("Current Device");
  await waitFor(() =>
    expect(desktop.state.workspace?.assistant.state.connection).toBe(
      "connected",
    ),
  );
  return { ...f, app, desktop };
}
it("keeps six ordered primary destinations, secondary memory and the existing workspace through layout and keyboard transitions", async () => {
  const f = await fixture(),
    workspace = f.desktop.state.workspace!,
    sidebar = f.app.container.querySelector(".rss-sidebar")!;
  expect(
    within(sidebar as HTMLElement)
      .getAllByRole("button")
      .filter((button) => button.classList.contains("rss-primary-button"))
      .map((button) => button.getAttribute("aria-label")),
  ).toEqual([
    "主页",
    "AI 助手",
    "软件中心",
    "工具中心",
    "系统设置",
    "任务中心",
  ]);
  fireEvent.click(
    within(sidebar as HTMLElement).getByRole("button", {
      name: "软件中心",
    }),
  );
  fireEvent.click(
    await within(sidebar as HTMLElement).findByRole("button", {
      name: "更新",
    }),
  );
  fireEvent.click(
    within(sidebar as HTMLElement).getByRole("button", {
      name: "主页",
    }),
  );
  fireEvent.click(
    within(sidebar as HTMLElement).getByRole("button", {
      name: "软件中心",
    }),
  );
  expect(workspace.navigation.state.selected.software).toBe("updates");
  const sequence = [];
  for (let i = 0; i < 4; i++) {
    fireEvent.click(screen.getByRole("button", { name: "切换侧栏布局" }));
    sequence.push(workspace.navigation.state.layoutStep);
  }
  expect(sequence).toEqual([1, 2, 3, 0]);
  fireEvent.keyDown(document, { key: "b", ctrlKey: true });
  expect(workspace.navigation.state.layoutStep).toBe(3);
  fireEvent.keyDown(document, { key: "b", ctrlKey: true });
  expect(workspace.navigation.state.layoutStep).toBe(0);
  expect(f.desktop.state.workspace).toBe(workspace);
  expect(f.input.assistantServices!.connect).toHaveBeenCalledOnce();
  expect(f.execute).not.toHaveBeenCalled();
  f.app.unmount();
});
it("projects home observations without collecting or launching, and hands the original draft to the assistant", async () => {
  const f = await fixture();
  await screen.findByRole("button", { name: "查看 Workspace" });
  expect(screen.getByText("来自本机安装清单；尚无使用频次记录。")).toBeTruthy();
  expect(screen.getByText("进程已结束，效果待核实")).toBeTruthy();
  expect(screen.queryByText("结果已核实")).toBeNull();
  const input = screen.getByRole("textbox", { name: "主页助手草稿" });
  fireEvent.change(input, { target: { value: "请解释当前设备状态" } });
  fireEvent.click(screen.getByRole("button", { name: "在 AI 助手中继续" }));
  expect(f.desktop.state.workspace!.navigation.state.page).toBe("assistant");
  expect(f.desktop.state.workspace!.assistant.draft.get()).toBe(
    "请解释当前设备状态",
  );
  expect(f.submit).not.toHaveBeenCalled();
  expect(f.querySubmit).not.toHaveBeenCalled();
  expect(f.execute).not.toHaveBeenCalled();
  f.app.unmount();
});
it("search locates current-subject records using keyboard and never executes the result", async () => {
  const f = await fixture();
  await screen.findByRole("button", { name: "查看 Workspace" });
  fireEvent.keyDown(document, { key: "k", ctrlKey: true });
  const search = await screen.findByRole("combobox", {
    name: "搜索应用、工具和设置",
  });
  fireEvent.change(search, { target: { value: "Workspace" } });
  fireEvent.keyDown(search, { key: "Enter" });
  expect(f.desktop.state.workspace!.navigation.state.page).toBe("software");
  expect(f.desktop.state.workspace!.navigation.state.selected.software).toBe(
    "installed",
  );
  expect(f.desktop.state.workspace!.navigation.state.target?.kind).toBe(
    "resource",
  );
  expect(f.execute).not.toHaveBeenCalled();
  f.app.unmount();
});
it("clears login secrets before awaiting the authority, retires private views and creates a fresh subject-bound workspace", async () => {
  const f = await fixture(),
    old = f.desktop.state.workspace!;
  old.assistant.draft.set("OLD PRIVATE DRAFT");
  await act(async () => {
    f.desktop.state.workspace!.navigation.select("settings", "account");
  });
  fireEvent.click(screen.getByRole("button", { name: "登录企业账号" }));
  const password = await screen.findByLabelText("企业密码");
  fireEvent.change(screen.getByLabelText("企业账号"), {
    target: { value: "new-person" },
  });
  fireEvent.change(password, { target: { value: "TOP-SECRET" } });
  fireEvent.click(screen.getByRole("button", { name: "登录" }));
  await waitFor(() => expect((password as HTMLInputElement).value).toBe(""));
  expect(f.desktop.state.workspace).toBeUndefined();
  expect(old.assistant.state.drafts.size).toBe(0);
  await waitFor(() =>
    expect(f.desktop.state.workspace?.identity.user.displayName).toBe(
      "new-person",
    ),
  );
  expect(f.desktop.state.workspace).not.toBe(old);
  expect(f.desktop.state.workspace!.assistant.draft.get()).toBe("");
  expect(f.app.container.textContent).not.toContain("TOP-SECRET");
  expect(f.clients[0].close).toHaveBeenCalledOnce();
  f.app.unmount();
});
it("keeps registration and MDM unknown observations independent", async () => {
  const f = await fixture();
  expect(f.desktop.fixture?.account).toBe(f.input.environment!.account);
  await act(async () => {
    f.desktop.state.workspace!.navigation.select("settings", "registration");
  });
  expect(screen.getByText("MDM 状态未知")).toBeTruthy();
  expect(screen.queryByText("由当前组织管理")).toBeNull();
  expect(screen.getAllByText("尚未注册").length).toBeGreaterThan(0);
  f.app.unmount();
});

it("clears unsubmitted account credentials on passive generation change and identity invalidation", async () => {
  const f = await fixture();
  await act(async () => {
    f.desktop.state.workspace!.navigation.select("settings", "account");
  });
  fireEvent.click(screen.getByRole("button", { name: "登录企业账号" }));
  const password = await screen.findByLabelText("企业密码"),
    login = screen.getByLabelText("企业账号");
  fireEvent.change(login, { target: { value: "private-person" } });
  fireEvent.change(password, { target: { value: "UNSUBMITTED SECRET" } });
  await act(async () => {
    f.desktop.account.user = {
      ...f.current(),
      generation: "passive-new-generation",
    };
  });
  expect((password as HTMLInputElement).value).toBe("");
  expect((login as HTMLInputElement).value).toBe("");
  fireEvent.change(password, { target: { value: "ANOTHER SECRET" } });
  await act(async () => {
    f.desktop.account.user = undefined;
  });
  expect((password as HTMLInputElement).value).toBe("");
  f.app.unmount();
});
it("changes appearance through the existing desktop owner without replacing the workspace", async () => {
  const f = await fixture(),
    workspace = f.desktop.state.workspace;
  await act(async () => {
    workspace!.navigation.select("settings", "appearance");
  });
  expect(screen.getByRole("combobox", { name: "界面主题" })).toBeTruthy();
  await act(async () => {
    f.desktop.setTheme("dark");
  });
  expect(
    f.app.container.querySelector(".rss-ui")?.getAttribute("data-theme"),
  ).toBe("dark");
  expect(f.desktop.state.workspace).toBe(workspace);
  f.app.unmount();
});

it("finds user settings through global search without adding them to primary navigation or executing", async () => {
  for (const [title, section] of [
    ["账号与组织", "account"],
    ["外观与体验", "appearance"],
    ["设备注册与 MDM", "registration"],
  ]) {
    const f = await fixture();
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });
    const search = await screen.findByRole("combobox", {
      name: "搜索应用、工具和设置",
    });
    fireEvent.change(search, { target: { value: title } });
    await screen.findByRole("option", { name: new RegExp(title) });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(f.desktop.state.workspace!.navigation.state.page).toBe("settings");
    expect(f.desktop.state.workspace!.navigation.state.selected.settings).toBe(
      section,
    );
    await waitFor(() =>
      expect(f.desktop.state.workspace!.navigation.state.searchOpen).toBe(
        false,
      ),
    );
    expect(f.execute).not.toHaveBeenCalled();
    expect(f.submit).not.toHaveBeenCalled();
    f.app.unmount();
  }
});
it("shows a first task detail failure and retries the same original reference from the open drawer", async () => {
  let detail!: ReturnType<typeof vi.fn>;
  const f = await fixture((f) => {
    const port = f.input.environment!.local.tasks!,
      read = port.detail;
    detail = vi.fn(read).mockRejectedValueOnce(Error("temporary unavailable"));
    port.detail = detail;
  });
  fireEvent.click(await screen.findByRole("button", { name: /diagnose/ }));
  await waitFor(() =>
    expect(f.desktop.state.workspace!.tasks.state.detailError).not.toBe(""),
  );
  const dialog = await screen.findByRole("dialog");
  expect(
    within(dialog).getByRole("heading", { name: "任务详情" }),
  ).toBeTruthy();
  await within(dialog).findByText("原任务不可访问或状态未确认，请重试读取");
  const original = f.desktop.state.workspace!.tasks.state.requested;
  expect(original).not.toBeNull();
  expect(f.desktop.state.workspace!.tasks.state.selected).toBeNull();
  const retry = await within(dialog).findByRole("button", {
    name: "查询原请求",
  });
  await waitFor(() =>
    expect((retry as HTMLButtonElement).disabled).toBe(false),
  );
  fireEvent.click(retry);
  await waitFor(() =>
    expect(f.desktop.state.workspace!.tasks.state.selected).not.toBeNull(),
  );
  expect(detail).toHaveBeenCalledTimes(2);
  expect(detail.mock.calls.map(([reference]) => reference)).toEqual([
    original,
    original,
  ]);
  expect(f.execute).not.toHaveBeenCalled();
  f.app.unmount();
});
it("stops only the registration wait and resumes the same retained grant", async () => {
  let rejectWait!: (error: Error) => void;
  let status = {
    phase: "unregistered",
    registered: false,
    credential: "absent",
    grantId: null,
  } as import("../settings/service-contract").RegistrationStatus;
  const request = vi.fn(async () => {
    if (!status.grantId) {
      status = {
        ...status,
        phase: "awaitingAuthorization",
        grantId: "original-grant",
      };
      return new Promise<typeof status>((_, reject) => {
        rejectWait = reject;
      });
    }
    return status;
  });
  const f = await fixture((f) => {
    const service = f.input.environment!.service;
    service.registration = async () => structuredClone(status);
    service.requestRegistration = request;
    service.cancelRegistration = async () =>
      rejectWait(Error("registration_cancelled"));
  });
  await act(async () => {
    await f.desktop.login({
      organizationId: "qinghe",
      login: "person",
      password: "fixture",
    });
  });
  await act(async () => {
    f.desktop.state.workspace!.navigation.select("settings", "registration");
  });
  fireEvent.click(await screen.findByRole("button", { name: "注册这台设备" }));
  fireEvent.click(await screen.findByRole("button", { name: "停止等待注册" }));
  const resume = await screen.findByRole("button", { name: "继续核验原注册" });
  expect(screen.getByText("original-grant")).toBeTruthy();
  fireEvent.click(resume);
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
  await screen.findByRole("button", { name: "继续核验原注册" });
  expect(screen.getByText("original-grant")).toBeTruthy();
  expect(status.registered).toBe(false);
  f.app.unmount();
});
