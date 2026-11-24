import { render, screen, fireEvent } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { EmployeeRecovery } from "./EmployeeRecovery";
import type { DeviceAccessPort } from "../self-service/native";

it("does not offer a mutation when the native session has no matching binding", async () => {
  const port: DeviceAccessPort = vi.fn(async () => ({
    kind: "ownBindings" as const,
    bindings: [],
    device: "3a0ea62c-fa31-4ba0-a8c5-b744543192bc",
    applicationAvailable: false,
  }));
  render(<EmployeeRecovery port={port} />);
  fireEvent.click(screen.getByText("刷新本人绑定"));
  expect(
    await screen.findByText("当前企业账号与本机账号没有可管理的绑定。"),
  ).toBeTruthy();
  expect(screen.queryByText("修复本人注册")).toBeNull();
  expect(screen.queryByText("登录 RSS 员工自助 Web")).toBeNull();
});
