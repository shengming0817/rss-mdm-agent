import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiProvider } from "@rss-mdm-agent/ui";
import { proxy } from "valtio/vanilla";
import { proxyMap } from "valtio/vanilla/utils";
import type { SettingsResponse, SettingsTarget } from "../self-service/types";
import { SettingsEditor } from "./SettingsEditor";
import type { SystemSettingsController } from "./controller";
it("preserves a dirty proxy draft and its original revision after background observations change", () => {
  const save = vi.fn(async () => undefined),
    owner = {
      save,
      state: proxy({ observations: proxyMap(), pending: proxyMap() }),
    } as unknown as SystemSettingsController,
    target = {
      id: "wifi",
      name: "Wi-Fi",
      fields: [
        {
          key: "http",
          value: { enabled: false, host: "", port: 0 },
          writable: true,
          quality: "complete" as const,
          reason: null,
        },
      ],
      options: [],
      reason: null,
    };
  const ui = (revision: string) => (
    <UiProvider>
      <SettingsEditor
        owner={owner}
        target={target}
        section="network"
        revision={revision}
        disabled={false}
      />
    </UiProvider>
  );
  const app = render(ui("original"));
  fireEvent.change(screen.getByRole("textbox", { name: "HTTP 代理服务器" }), {
    target: { value: "proxy.example" },
  });
  app.rerender(ui("new-observation"));
  expect(
    (
      screen.getByRole("textbox", {
        name: "HTTP 代理服务器",
      }) as HTMLInputElement
    ).value,
  ).toBe("proxy.example");
  fireEvent.click(screen.getByRole("button", { name: "保存更改" }));
  expect(save).toHaveBeenCalledWith(
    expect.objectContaining({
      kind: "proxy",
      http: expect.objectContaining({ host: "proxy.example" }),
    }),
    "original",
  );
});

it("starts each clean draft from the current observation and ends only its acknowledged applied request", async () => {
  const save = vi.fn(async () => "original-request"),
    state = proxy({
      observations: proxyMap<string, SettingsResponse>(),
      pending: proxyMap(),
    }),
    owner = { save, state } as unknown as SystemSettingsController,
    target = {
      id: "ac",
      name: "电源",
      fields: [
        {
          key: "sleepMinutes",
          value: 30,
          writable: true,
          quality: "complete" as const,
          reason: null,
        },
      ],
      options: [],
      reason: null,
    };
  const ui = (revision: string) => (
    <UiProvider>
      <SettingsEditor
        owner={owner}
        target={target}
        section="power"
        revision={revision}
        disabled={false}
      />
    </UiProvider>
  );
  const app = render(ui("R1"));
  app.rerender(ui("R2"));
  fireEvent.change(
    screen.getByRole("spinbutton", { name: "电源 系统睡眠（分钟）" }),
    { target: { value: "20" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "保存更改" }));
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({ sleepMinutes: 20 }),
      "R2",
    ),
  );
  await act(async () => {});
  const result = {
    request: "other-request",
    finished: true,
    effect: "applied",
  } as SettingsResponse;
  act(() => {
    state.observations.set("power", result);
  });
  expect(screen.getByText("有尚未保存的修改")).toBeTruthy();
  act(() => {
    state.observations.set("power", {
      ...result,
      request: "original-request",
      effect: "conflict",
    });
  });
  expect(screen.getByText("有尚未保存的修改")).toBeTruthy();
  app.rerender(ui("R3"));
  act(() => {
    state.observations.set("power", { ...result, request: "original-request" });
  });
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "保存更改" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true),
  );
  fireEvent.change(
    screen.getByRole("spinbutton", { name: "电源 系统睡眠（分钟）" }),
    { target: { value: "15" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "保存更改" }));
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(
      expect.objectContaining({ sleepMinutes: 15 }),
      "R3",
    ),
  );
});

function editorFixture(section: "network" | "display", target: SettingsTarget) {
  const save = vi.fn(async () => undefined),
    owner = {
      save,
      state: proxy({ observations: proxyMap(), pending: proxyMap() }),
    } as unknown as SystemSettingsController;
  const ui = (revision = "R1", current = target, disabled = false) => (
    <UiProvider>
      <SettingsEditor
        owner={owner}
        target={current}
        section={section}
        revision={revision}
        disabled={disabled}
      />
    </UiProvider>
  );
  return { save, owner, ui, app: render(ui()) };
}
const field = (
  key: string,
  value: SettingsTarget["fields"][number]["value"],
  writable = true,
) => ({ key, value, writable, quality: "complete" as const, reason: null });
const settingsTarget = (fields: SettingsTarget["fields"]): SettingsTarget => ({
  id: "target",
  name: "目标",
  fields,
  options: [
    { id: "default", label: "默认" },
    { id: "large", label: "较大" },
  ],
  reason: null,
});
it("edits a bypass-only target as an exact delta, retaining raw newlines and the original revision across observations", async () => {
  const target = settingsTarget([
    field("http", { enabled: true, host: "managed", port: 80 }, false),
    field("bypass", ["localhost", "*.internal"]),
  ]);
  const f = editorFixture("network", target),
    input = screen.getByRole("textbox", { name: "目标 绕过代理的地址" });
  expect((input as HTMLTextAreaElement).value).toBe("localhost\n*.internal");
  const text = " localhost\n\n *.corp \n";
  fireEvent.change(input, { target: { value: text } });
  expect((input as HTMLTextAreaElement).value).toBe(text);
  f.app.rerender(
    f.ui("R2", settingsTarget([field("bypass", ["new-observation"])])),
  );
  expect((input as HTMLTextAreaElement).value).toBe(text);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "保存更改" })),
  );
  expect(f.save).toHaveBeenCalledExactlyOnceWith(
    {
      kind: "proxy",
      target: "target",
      http: null,
      https: null,
      socks: null,
      bypass: ["localhost", "*.corp"],
    },
    "R1",
  );
});
it("submits an explicit empty bypass list and restores the observed entries only when the draft is reset", async () => {
  const f = editorFixture(
    "network",
    settingsTarget([field("bypass", ["localhost"])]),
  );
  const input = screen.getByRole("textbox", { name: "目标 绕过代理的地址" });
  fireEvent.change(input, { target: { value: "" } });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "保存更改" })),
  );
  expect(f.save).toHaveBeenCalledWith(
    expect.objectContaining({ bypass: [], http: null }),
    "R1",
  );
  fireEvent.click(screen.getByRole("button", { name: "还原草稿" }));
  expect((input as HTMLTextAreaElement).value).toBe("localhost");
});
it.each([true, false])(
  "leaves unchanged bypass entries out of a protocol-only delta (bypass writable=%s)",
  async (writable) => {
    const f = editorFixture(
      "network",
      settingsTarget([
        field("http", { enabled: false, host: "", port: 0 }),
        field("bypass", ["managed"], writable),
      ]),
    );
    const bypass = screen.getByRole("textbox", {
      name: "目标 绕过代理的地址",
    }) as HTMLTextAreaElement;
    expect(bypass.disabled).toBe(!writable);
    if (!writable)
      fireEvent.change(bypass, { target: { value: "must-not-submit" } });
    fireEvent.change(screen.getByRole("textbox", { name: "HTTP 代理服务器" }), {
      target: { value: "proxy.example" },
    });
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "保存更改" })),
    );
    expect(f.save).toHaveBeenCalledWith(
      expect.objectContaining({
        bypass: null,
        http: expect.objectContaining({ host: "proxy.example" }),
      }),
      "R1",
    );
  },
);
it("submits explicit negative display coordinates, retains the other coordinate and omits the read-only mode", async () => {
  const f = editorFixture(
    "display",
    settingsTarget([
      field("mode", "default", false),
      field("origin", { x: 80, y: 10 }),
    ]),
  );
  expect(
    (
      screen.getByRole("combobox", {
        name: "目标 显示模式",
      }) as HTMLInputElement
    ).disabled,
  ).toBe(true);
  fireEvent.change(
    screen.getByRole("spinbutton", { name: "目标 桌面横坐标" }),
    { target: { value: "-1920" } },
  );
  f.app.rerender(
    f.ui(
      "R2",
      settingsTarget([
        field("mode", "default", false),
        field("origin", { x: 500, y: 500 }),
      ]),
    ),
  );
  expect(
    (
      screen.getByRole("spinbutton", {
        name: "目标 桌面纵坐标",
      }) as HTMLInputElement
    ).value,
  ).toBe("10");
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "保存更改" })),
  );
  expect(f.save).toHaveBeenCalledExactlyOnceWith(
    {
      kind: "display",
      target: "target",
      mode: null,
      origin: { x: -1920, y: 10 },
    },
    "R1",
  );
});
it("requires both explicit coordinates and blocks a draft whose field permission is later revoked", async () => {
  const f = editorFixture(
    "display",
    settingsTarget([field("mode", "default"), field("origin", null)]),
  );
  fireEvent.change(
    screen.getByRole("spinbutton", { name: "目标 桌面横坐标" }),
    { target: { value: "-100" } },
  );
  const save = screen.getByRole("button", {
    name: "保存更改",
  }) as HTMLButtonElement;
  expect(save.disabled).toBe(true);
  expect(screen.getByText("请填写完整的整数坐标后再保存。")).toBeTruthy();
  fireEvent.click(save);
  expect(f.save).not.toHaveBeenCalled();
  fireEvent.change(
    screen.getByRole("spinbutton", { name: "目标 桌面纵坐标" }),
    { target: { value: "0" } },
  );
  expect(save.disabled).toBe(false);
  f.app.rerender(
    f.ui(
      "R2",
      settingsTarget([
        field("mode", "default"),
        field("origin", { x: 0, y: 0 }, false),
      ]),
    ),
  );
  expect(
    (
      screen.getByRole("spinbutton", {
        name: "目标 桌面横坐标",
      }) as HTMLInputElement
    ).disabled,
  ).toBe(true);
  expect(save.disabled).toBe(true);
  fireEvent.click(save);
  expect(f.save).not.toHaveBeenCalled();
});
