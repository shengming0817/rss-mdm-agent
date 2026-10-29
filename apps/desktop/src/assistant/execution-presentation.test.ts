import { expect, it } from "vitest";
import { taskPresentation } from "./execution-presentation";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type { ExecutionTaskDetails } from "@rss-mdm-agent/execution-bindings/tasks";

function task(
  phase: ExecutionTaskDetails["status"]["phase"],
  assessment: ExecutionTaskDetails["status"]["assessment"] = null,
) {
  const value = structuredClone(
    fixtures.outcomeUnknown,
  ) as ExecutionTaskDetails;
  value.action.initiator = {
    kind: "backend",
    trigger: structuredClone(fixtures.proposed.trigger),
  } as ExecutionTaskDetails["action"]["initiator"];
  value.status.phase = phase;
  value.status.assessment = assessment;
  return value;
}
it.each([
  ["satisfied", "已核实符合预期"],
  ["notSatisfied", "已核实未达预期"],
  ["noEffect", "已核实无效果"],
  ["unknown", "效果仍需核对"],
] as const)("distinguishes verified assessment %s", (assessment, label) => {
  expect(taskPresentation(task("verified", assessment)).label).toBe(label);
});
it("does not infer verified success from cancellation, process exit or a software observation", () => {
  const value = task("executionEnded", "unknown");
  value.status.cancelRequested = true;
  value.status.stopOutcome = "acknowledged";
  value.status.software = "desiredStateObserved";
  const result = taskPresentation(value);
  expect(result.label).toContain("等待效果验证");
  expect(result.stage).toContain("不代表后台活动已终止");
  expect(result.cancel).toContain("已请求取消");
});
