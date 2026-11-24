import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { DeviceAccessPort } from "../self-service/native";
import { WindowsLoginSetup } from "./WindowsLoginSetup";

it("shows the employee's pending task while Windows is awaiting a reboot", async () => {
  const port = vi.fn<DeviceAccessPort>(async (op) => {
    if (op.kind === "windowsStatus") {
      return {
        kind: "windowsStatus",
        component: "rebootRequired",
        factorActive: null,
        signInRequired: false,
        hello: {
          availability: "unavailable",
          desiredEnabled: null,
          retired: false,
          replacementRequired: false,
          facts: null,
        },
      };
    }
    if (op.kind === "tasks")
      return {
        kind: "tasks",
        tasks: [{ task: "task", binding: "binding", createName: "alice" }],
      };
    throw Error("unexpected setup request");
  });
  render(<WindowsLoginSetup port={port} />);
  fireEvent.click(screen.getByText("刷新组件状态和待设置任务"));
  const setup = await screen.findByRole("button", {
    name: "为新账号 alice 设置验证器",
  });
  expect(setup.hasAttribute("disabled")).toBe(true);
  expect(screen.getByText("需要重启 Windows 后才能设置登录因子")).toBeTruthy();
});
