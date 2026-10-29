import { expect, it } from "vitest";
import { snapshot } from "valtio/vanilla";
import { createNavigation, layouts } from "./navigation";
it("remembers every module section and preserves the direction of both rail positions", () => {
  const nav = createNavigation();
  for (const [page, section] of [
    ["assistant", "history"],
    ["software", "updates"],
    ["system", "power"],
    ["tasks", "management"],
    ["settings", "appearance"],
  ] as const)
    expect(nav.select(page, section)).toBe(true);
  const before = snapshot(nav.state);
  nav.state.layoutStep = 0;
  const cycle = [];
  for (let i = 0; i < 5; i++) {
    cycle.push(layouts[nav.state.layoutStep]);
    nav.cycle();
  }
  expect(cycle).toEqual(["hidden", "rail", "full", "rail", "hidden"]);
  expect(nav.state.selected).toEqual(before.selected);
  expect(nav.state.page).toBe("settings");
  nav.state.layoutStep = 3;
  nav.hideOrRestore();
  nav.hideOrRestore();
  nav.cycle();
  expect(layouts[nav.state.layoutStep]).toBe("hidden");
  nav.state.layoutStep = 1;
  nav.hideOrRestore();
  nav.hideOrRestore();
  nav.cycle();
  expect(layouts[nav.state.layoutStep]).toBe("full");
  expect(nav.select("software", "untrusted")).toBe(false);
  expect(nav.state.selected.software).toBe("updates");
});
it("keeps the history panel open through module and session selection", () => {
  const nav = createNavigation();
  nav.toggleHistory();
  nav.select("software");
  nav.select("assistant", "new");
  expect(nav.state.historyOpen).toBe(true);
  nav.closeHistory();
  expect(nav.state.historyOpen).toBe(false);
});

it("restores the last visible direction after cycling to hidden, without resetting the selected page", () => {
  const nav = createNavigation();
  nav.select("software", "updates");
  nav.cycle();
  nav.cycle();
  nav.cycle();
  expect(nav.state.layoutStep).toBe(0);
  expect(nav.state.restoreStep).toBe(3);
  nav.hideOrRestore();
  expect(nav.state.layoutStep).toBe(3);
  nav.cycle();
  expect(nav.state.layoutStep).toBe(0);
  nav.hideOrRestore();
  expect(nav.state.layoutStep).toBe(3);
  nav.cycle();
  nav.cycle();
  nav.hideOrRestore();
  nav.hideOrRestore();
  expect(nav.state.layoutStep).toBe(1);
  nav.cycle();
  expect(nav.state.layoutStep).toBe(2);
  nav.hideOrRestore();
  nav.hideOrRestore();
  expect(nav.state.layoutStep).toBe(2);
  nav.cycle();
  expect(nav.state.layoutStep).toBe(3);
  expect(nav.state.page).toBe("software");
  expect(nav.state.selected.software).toBe("updates");
});
