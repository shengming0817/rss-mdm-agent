import { render, screen, fireEvent } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { EmployeeRecovery } from "./EmployeeRecovery";
import {
  nativeEmployeeChange,
  type DeviceAccessPort,
} from "../self-service/native";

vi.mock("../self-service/native", () => ({ nativeEmployeeChange: vi.fn() }));

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

it("resumes the original reset after restart even when the server has no receipt", async () => {
  const intent = {
    binding: "82c62db3-ce5e-49fb-a688-ba1128a32a08",
    revision: 3,
    change: "reset_factors",
    stage: "open",
  };
  vi.mocked(nativeEmployeeChange)
    .mockResolvedValueOnce({
      kind: "stored",
      intent: { ...intent, change: "reset_factors", stage: "open" },
    })
    .mockRejectedValueOnce({ code: "operation_absent" });
  render(
    <EmployeeRecovery
      port={vi.fn().mockResolvedValue({
        kind: "ownBindings",
        bindings: [],
        applicationAvailable: false,
      })}
    />,
  );
  fireEvent.click(screen.getByText("刷新本人绑定"));
  fireEvent.click(await screen.findByText("查询原操作"));
  expect(await screen.findByText(/服务端尚未登记这次操作/)).toBeTruthy();
  expect(nativeEmployeeChange).toHaveBeenCalledWith({
    ...intent,
    stage: "status",
  });
});
