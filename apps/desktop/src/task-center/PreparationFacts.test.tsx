import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { PolicyFacts } from "./PreparationFacts";
import { policyRecord } from "../../../../tests/self-service/support";
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
