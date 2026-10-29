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
  return { app, workspace, page, pages };
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
  await screen.findByRole("dialog");
  expect(
    f.pages.events.filter((value) => value.operation === "software_prepare"),
  ).toHaveLength(1);
  expect(
    f.pages.events.some((value) => value.operation === "software_submit"),
  ).toBe(false);
  const dialog = within(screen.getByRole("dialog"));
  fireEvent.click(dialog.getByRole("button", { name: "确认打开" }));
  await waitFor(() =>
    expect(
      f.pages.events.filter((value) => value.operation === "software_submit"),
    ).toHaveLength(1),
  );
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
