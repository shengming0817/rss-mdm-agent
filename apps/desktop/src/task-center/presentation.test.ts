import { expect, it } from "vitest";
import { taskDetail } from "../../../../tests/self-service/support";
import { presentTask } from "./presentation";
it.each(["satisfied", "notSatisfied", "noEffect", "unknown"] as const)(
  "does not turn verified/%s into an unconditional success color",
  (assessment) => {
    const task = taskDetail();
    if (task.facts.kind !== "execution") throw Error("fixture");
    task.stage = "verified";
    task.facts.value.status.phase = "verified";
    task.facts.value.status.assessment = assessment;
    const view = presentTask(task);
    expect(view.success).toBe(assessment === "satisfied");
    expect(view.color === "green").toBe(assessment === "satisfied");
    if (assessment !== "satisfied")
      expect(view.label).not.toContain("符合预期");
  },
);
it("keeps acknowledged cancellation, execution exit and independent desired-state observations separate", () => {
  const task = taskDetail();
  if (task.facts.kind !== "execution") throw Error("fixture");
  Object.assign(task.facts.value.status, {
    phase: "executionEnded",
    assessment: null,
    cancelRequested: true,
    stopOutcome: "acknowledged",
    software: "desiredStateObserved",
  });
  expect(presentTask(task)).toMatchObject({ success: false, attention: true });
  expect(presentTask(task).description).toContain("终止及效果仍由执行服务核对");
});
