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
import type { SettingsResponse } from "../self-service/types";
import { SettingsEditor } from "./SettingsEditor";
import type { SystemSettingsController } from "./controller";
it("preserves a dirty proxy draft and its original revision after background observations change", () => {
  const save = vi.fn(async () => undefined),
    owner = {
      save,
      state: proxy({ observations: proxyMap() }),
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
    state = proxy({ observations: proxyMap<string, SettingsResponse>() }),
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
