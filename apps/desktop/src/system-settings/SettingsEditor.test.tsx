import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { UiProvider } from "@rss-mdm-agent/ui";
import { SettingsEditor } from "./SettingsEditor";
import type { SystemSettingsController } from "./controller";
it("preserves a dirty proxy draft and its original revision after background observations change", () => {
  const save = vi.fn(),
    owner = { save } as unknown as SystemSettingsController,
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
