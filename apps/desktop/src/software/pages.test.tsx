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
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => false,
  invoke: vi.fn(),
}));
async function mount(scene = "normal") {
  const fixture = shellFixture(),
    pages = configurePageFixture(fixture.input, scene);
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
  const page = (name: string) =>
    within(
      app.container.querySelector(
        `.rss-page-viewport[aria-label="${name}"]`,
      ) as HTMLElement,
    );
  return { app, workspace, page, pages, fixture };
}
it("uses published categories and full detail while installed facts retain protected actions and explicit original preparation", async () => {
  const f = await mount();
  act(() => {
    f.workspace.navigation.select("software", "discover");
  });
  await f.page("软件中心").findByRole("button", { name: "工作空间" });
  expect(
    f.app.container.querySelectorAll(".rss-store-editorial > section"),
  ).toHaveLength(2);
  expect(f.pages.events).toEqual([]);
  act(() => {
    f.workspace.navigation.select("software", "category:创意设计");
  });
  await f.page("软件中心").findByRole("button", { name: "创作画板" });
  expect(
    f.page("软件中心").queryByRole("button", { name: "工作空间" }),
  ).toBeNull();
  fireEvent.click(f.page("软件中心").getByRole("button", { name: "创作画板" }));
  await f.page("软件中心").findByRole("heading", { name: "创作画板" });
  fireEvent.click(f.page("软件中心").getByRole("tab", { name: "版本记录" }));
  expect(
    f.page("软件中心").getByText("发布目录未提供软件版本历史"),
  ).toBeTruthy();
  act(() => {
    f.workspace.navigation.select("software", "installed");
  });
  await f.page("软件中心").findByRole("button", { name: "Workspace" });
  const card = f.app.container.querySelector(
    ".rss-installed-card",
  ) as HTMLElement;
  expect(
    (within(card).getByRole("button", { name: "卸载" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  fireEvent.click(within(card).getByRole("button", { name: "打开" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "software_submit"),
    ).toHaveLength(1),
  );
  expect(
    f.pages.events.filter((value) => value.operation === "software_prepare"),
  ).toHaveLength(1);
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(f.workspace.navigation.state.page).toBe("software");
  f.app.unmount();
});
it("reports unavailable observations and missing capabilities honestly in offline and read-only pages", async () => {
  const f = await mount("offline");
  act(() => {
    f.workspace.navigation.select("software", "discover");
  });
  await f.page("软件中心").findByText("当前范围没有可见应用");
  expect(
    f.pages.events.some((value) => value.operation.includes("submit")),
  ).toBe(false);
  f.app.unmount();
  const readOnly = await mount("readonly");
  act(() => {
    readOnly.workspace.navigation.select("software", "installed");
  });
  await readOnly.page("软件中心").findByRole("button", { name: "Workspace" });
  expect(
    readOnly.app.container.querySelectorAll(
      ".rss-installed-card-actions button:disabled",
    ).length,
  ).toBeGreaterThan(0);
  readOnly.app.unmount();
});
it("retains system drafts across secondary navigation and rereads every overview scope after a failed observation", async () => {
  const f = await mount();
  act(() => {
    f.workspace.navigation.select("system", "network");
  });
  const input = await f
    .page("系统设置")
    .findByRole("textbox", { name: "HTTP 代理服务器" });
  await f.page("系统设置").findByText("192.0.2.24");
  fireEvent.change(input, { target: { value: "retained.example" } });
  act(() => {
    f.workspace.navigation.select("system", "display");
  });
  await f.page("系统设置").findByRole("heading", { name: "显示器" });
  act(() => {
    f.workspace.navigation.select("system", "network");
  });
  expect(
    (
      (await f
        .page("系统设置")
        .findByRole("textbox", { name: "HTTP 代理服务器" })) as HTMLInputElement
    ).value,
  ).toBe("retained.example");
  const read = vi.spyOn(f.workspace.system, "load");
  act(() => {
    f.workspace.system.state.errors.set("network", "读取失败");
    f.workspace.navigation.select("system", "overview");
  });
  await f.page("系统设置").findByRole("heading", { name: "Current Device" });
  await waitFor(() =>
    expect(
      (
        f
          .page("系统设置")
          .getByRole("button", { name: "重读状态" }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  read.mockClear();
  fireEvent.click(f.page("系统设置").getByRole("button", { name: "重读状态" }));
  expect(read.mock.calls.map((call) => call[0])).toEqual([
    "network",
    "storage",
    "power",
    "about",
  ]);
  f.app.unmount();
});

it("can explicitly re-prepare the same failed organization project with fresh fields and a new request", async () => {
  const f = await mount();
  const item = f.pages.published.find((value) => value.task.kind === "script")!;
  await act(async () => {
    await f.workspace.policies.select(item);
    f.workspace.navigation.showPolicy();
  });
  const dialog = () => within(screen.getByRole("dialog"));
  fireEvent.change(dialog().getByRole("textbox", { name: "interface" }), {
    target: { value: "en0" },
  });
  fireEvent.click(dialog().getByRole("button", { name: "确认并提交" }));
  await dialog().findByRole("button", { name: "查询原请求" });
  const original = f.workspace.policies.state.submission!.choice.request;
  f.pages.records.get(original)!.state = "failed";
  fireEvent.click(dialog().getByRole("button", { name: "查询原请求" }));
  fireEvent.click(await dialog().findByRole("button", { name: "重新准备" }));
  const input = await dialog().findByRole("textbox", { name: "interface" });
  expect((input as HTMLInputElement).value).toBe("");
  expect(
    f.pages.events.filter((value) => value.operation === "policy_invoke"),
  ).toHaveLength(1);
  fireEvent.change(input, { target: { value: "en1" } });
  fireEvent.click(dialog().getByRole("button", { name: "确认并提交" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "policy_invoke"),
    ).toHaveLength(2),
  );
  const choice = f.workspace.policies.state.submission!.choice;
  expect(choice.request).not.toBe(original);
  expect(choice.parameters).toMatchObject({ interface: "en1" });
  f.app.unmount();
});
it("ends a verified settings save before the next edit using the new CAS revision", async () => {
  const f = await mount();
  act(() => f.workspace.navigation.select("system", "network"));
  const input = await f
    .page("系统设置")
    .findByRole("textbox", { name: "HTTP 代理服务器" });
  fireEvent.change(input, { target: { value: "proxy.one.invalid" } });
  fireEvent.click(f.page("系统设置").getByRole("button", { name: "保存更改" }));
  await waitFor(() =>
    expect(
      (
        f
          .page("系统设置")
          .getByRole("button", { name: "保存更改" }) as HTMLButtonElement
      ).disabled,
    ).toBe(true),
  );
  await waitFor(() =>
    expect(f.workspace.system.state.observations.get("network")?.effect).toBe(
      "applied",
    ),
  );
  const revision =
    f.workspace.system.state.observations.get("network")!.snapshot!.revision;
  fireEvent.change(input, { target: { value: "proxy.two.invalid" } });
  fireEvent.click(f.page("系统设置").getByRole("button", { name: "保存更改" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "settings_change"),
    ).toHaveLength(2),
  );
  expect(
    f.workspace.system.state.observations.get("network")?.snapshot?.revision,
  ).not.toBe(revision);
  expect(f.workspace.system.state.errors.size).toBe(0);
  f.app.unmount();
});

it("installs an organization app with one click and keeps progress on the software page", async () => {
  const f = await mount("tasks");
  act(() => f.workspace.navigation.select("software", "discover"));
  const install = (
    await f.page("软件中心").findAllByRole("button", { name: "安装" })
  )[0];
  fireEvent.click(install);
  fireEvent.click(install);
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "policy_invoke"),
    ).toHaveLength(1),
  );
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(f.workspace.navigation.state.page).toBe("software");
  await f.page("软件中心").findByRole("button", { name: "查询进度" });
  expect(
    f.page("软件中心").queryByText(/固定资源|发布 ID|配置版本|原工具调用/),
  ).toBeNull();
  f.app.unmount();
});

it("confirms uninstall once and displays the result without a preparation drawer", async () => {
  const f = await mount();
  act(() => f.workspace.navigation.select("software", "installed"));
  await f.page("软件中心").findByRole("button", { name: "Workspace" });
  act(() => {
    for (const source of f.workspace.software.state.sources) {
      for (const item of source.items) {
        const uninstall = item.actions.find(
          (value) => value.action === "uninstall",
        );
        if (uninstall) {
          uninstall.available = true;
          uninstall.reason = null;
        }
      }
    }
  });
  const card = f.app.container.querySelector(
    ".rss-installed-card",
  ) as HTMLElement;
  await waitFor(() =>
    expect(
      (within(card).getByRole("button", { name: "卸载" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  fireEvent.click(within(card).getByRole("button", { name: "卸载" }));
  const dialog = within(await screen.findByRole("dialog"));
  expect(
    f.pages.events.filter((value) => value.operation === "software_prepare"),
  ).toHaveLength(0);
  fireEvent.click(dialog.getByRole("button", { name: "卸载" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "software_submit"),
    ).toHaveLength(1),
  );
  expect(f.workspace.navigation.state.page).toBe("software");
  expect(f.app.container.textContent).not.toContain("固定资源");
  f.app.unmount();
});

it("resumes the same local request after a lost preparation response", async () => {
  const f = await mount();
  const port = f.workspace.software.port!;
  const prepare = port.prepare.bind(port);
  vi.spyOn(port, "prepare").mockImplementationOnce(async (input) => {
    await prepare(input);
    throw Error("lost reply");
  });
  act(() => f.workspace.navigation.select("software", "installed"));
  await f.page("软件中心").findByRole("button", { name: "Workspace" });
  const card = f.app.container.querySelector(
    ".rss-installed-card",
  ) as HTMLElement;
  fireEvent.click(within(card).getByRole("button", { name: "打开" }));
  const resume = await within(card).findByRole("button", { name: "继续打开" });
  expect(
    f.pages.events.filter((value) => value.operation === "software_submit"),
  ).toHaveLength(0);
  fireEvent.click(resume);
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "software_submit"),
    ).toHaveLength(1),
  );
  const events = f.pages.events.filter((value) =>
    ["software_prepare", "software_submit"].includes(value.operation),
  );
  expect(new Set(events.map((value) => value.request)).size).toBe(1);
  f.app.unmount();
});

it("recovers a lost install acknowledgement so another app can be installed", async () => {
  const f = await mount("tasks");
  const port = f.fixture.input.selfServicePort!;
  const invoke = port.policy_invoke.bind(port);
  vi.spyOn(port, "policy_invoke").mockImplementationOnce(async (input) => {
    await invoke(input);
    throw Error("lost reply");
  });
  act(() => f.workspace.navigation.select("software", "discover"));
  const buttons = await f
    .page("软件中心")
    .findAllByRole("button", { name: "安装" });
  fireEvent.click(buttons[0]);
  await waitFor(
    () => expect(f.workspace.policies.state.status?.kind).toBe("preparation"),
    { timeout: 5000 },
  );
  fireEvent.click(buttons[1]);
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "policy_invoke"),
    ).toHaveLength(2),
  );
  expect(f.workspace.navigation.state.page).toBe("software");
  f.app.unmount();
});

it("keeps a completed app reinstallable after installing another app and ignores late old progress", async () => {
  const f = await mount("tasks");
  act(() => f.workspace.navigation.select("software", "discover"));
  const buttons = await f
    .page("软件中心")
    .findAllByRole("button", { name: "安装" });
  const card = buttons[0].closest(".rss-software-card") as HTMLElement;
  fireEvent.click(buttons[0]);
  await waitFor(() => expect(f.workspace.policies.state.busy).toBe(false));
  const original = f.workspace.policies.state.submission!.choice.request;
  const port = f.fixture.input.selfServicePort!;
  const status = port.policy_status.bind(port);
  vi.spyOn(port, "policy_status").mockImplementation(async (input) =>
    input.requestId === original
      ? {
          kind: "run",
          value: {
            requestId: original,
            runId: "run-a",
            attemptId: null,
            state: {
              execution: "succeeded",
              delivery: { kind: "queued" },
              cancellation: "none",
              deadline: 0,
              startedAt: 1,
            },
            availableAt: 0,
            deadline: 0,
            result: null,
          },
        }
      : status(input),
  );
  fireEvent.click(
    await within(card).findByRole("button", { name: "查询进度" }),
  );
  await within(card).findByRole("button", { name: "重新安装" });
  fireEvent.click(buttons[1]);
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "policy_invoke"),
    ).toHaveLength(2),
  );
  const detail = f.workspace.tasks.detail.bind(f.workspace.tasks);
  let resolveOld!: (value: Awaited<ReturnType<typeof detail>>) => void;
  const old = await detail({ kind: "request", request: original }, true);
  old.stage = "failed";
  vi.spyOn(f.workspace.tasks, "detail").mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
  );
  fireEvent.click(within(card).getByRole("button", { name: "查询进度" }));
  await waitFor(() => expect(resolveOld).toBeTruthy());
  fireEvent.click(within(card).getByRole("button", { name: "重新安装" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "policy_invoke"),
    ).toHaveLength(3),
  );
  const current = f.workspace.policies.state.submission!.choice.request;
  expect(current).not.toBe(original);
  await act(async () => resolveOld(old));
  expect(within(card).queryByText("准备失败")).toBeNull();
  expect(
    f.workspace.software.state.policyRequests.get(
      f.pages.published.filter((value) => value.task.kind === "software")[0]
        .policyId,
    ),
  ).toBe(current);
  f.app.unmount();
});
