import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { PolicyFacts } from "./PreparationFacts";
import { policyRecord, offer } from "../../../../tests/self-service/support";
it("reviews frozen scalar input while protected schemas never reveal their values", () => {
  const record = policyRecord();
  if (record.action.kind !== "policy" || record.action.task.kind !== "script")
    throw Error("fixture");
  record.action.task.parameterSchema = {
    type: "object",
    properties: { interface: { type: "string" } },
  };
  record.action.task.parameterSources = { interface: { kind: "input" } };
  record.action.choice.parameters = { interface: "en0" };
  const app = render(<PolicyFacts record={record} />);
  expect(screen.getByText(/en0；已冻结/)).toBeTruthy();
  record.action.task.parameterSchema = {
    type: "object",
    properties: { interface: { type: "string", writeOnly: true } },
  };
  record.action.choice.parameters = { interface: "private-never-render" };
  app.rerender(<PolicyFacts record={record} />);
  expect(app.container.textContent).not.toContain("private-never-render");
  expect(screen.getByText(/受保护输入不回显/)).toBeTruthy();
});

it.each([
  ["system", "系统执行身份"],
  ["user", "本机用户执行身份"],
] as const)(
  "shows trusted offered script identity %s before confirmation",
  (identity, label) => {
    const record = policyRecord();
    record.action = {
      kind: "offered",
      offer: {
        ...offer(),
        presentation: {
          title: "受控脚本",
          summary: { kind: "script", identity },
        },
      },
    };
    const app = render(<PolicyFacts record={record} />);
    expect(screen.getByText("脚本", { exact: true })).toBeTruthy();
    expect(screen.getByText(label, { exact: true })).toBeTruthy();
    expect(app.container.textContent).not.toContain(
      identity === "system" ? "本机用户执行身份" : "系统执行身份",
    );
  },
);
