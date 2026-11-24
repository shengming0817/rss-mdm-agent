import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { DeviceAccessPort } from "../self-service/native";
import { MacAccountSetup } from "./MacAccountSetup";

it("sends only the displayed task and attempt to the native dialog", async () => {
  const port = vi.fn<DeviceAccessPort>(async (op) => {
    if (op.kind === "tasks")
      return {
        kind: "tasks",
        tasks: [{ task: "t", attempt: "a", binding: "b", createName: "alice" }],
      };
    return { kind: "prepared" };
  });
  render(<MacAccountSetup port={port} />);
  fireEvent.click(screen.getByRole("button", { name: "刷新建号任务" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "为 alice 设置密码" }),
  );
  await screen.findByText("密码已提交给本机服务，账号结果由原操作回报。");
  expect(port.mock.calls[1]).toEqual([
    { kind: "prepareMac", task: "t", attempt: "a" },
  ]);
  expect(document.querySelector('input[type="password"]')).toBeNull();
});
