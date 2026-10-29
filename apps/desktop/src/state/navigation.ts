import { proxy } from "valtio/vanilla";

export const modules = [
  ["home", "主页"],
  ["assistant", "AI 助手"],
  ["software", "软件中心"],
  ["tools", "工具中心"],
  ["system", "系统设置"],
  ["tasks", "任务中心"],
] as const;
export type ModuleId = (typeof modules)[number][0];
export type PageId = ModuleId | "settings";
export const sections = {
  home: ["overview"],
  assistant: ["new", "history"],
  software: [
    "discover",
    "installed",
    "updates",
    "category:效率办公",
    "category:创意设计",
    "category:网络工具",
    "category:开发工具",
    "category:系统工具",
  ],
  tools: ["all", "network", "maintenance", "security"],
  system: [
    "overview",
    "network",
    "display",
    "sound",
    "power",
    "storage",
    "security",
    "about",
  ],
  tasks: ["active", "history", "management"],
  settings: [
    "account",
    "registration",
    "connection",
    "personal-vault",
    "diagnostics",
    "appearance",
    "about",
  ],
} as const;
/** Two rail steps preserve the direction of the approved four-step cycle. */
export const layouts = ["hidden", "rail", "full", "rail"] as const;
export type LayoutStep = 0 | 1 | 2 | 3;
export function createNavigation() {
  const state = proxy({
    page: "home" as PageId,
    selected: Object.fromEntries(
      Object.entries(sections).map(([id, values]) => [id, values[0]]),
    ) as Record<PageId, string>,
    layoutStep: 1 as LayoutStep,
    restoreStep: 1 as Exclude<LayoutStep, 0>,
    historyOpen: false,
    searchOpen: false,
  });
  return {
    state,
    select(page: PageId, section?: string) {
      if (!Object.hasOwn(sections, page)) return false;
      if (
        section !== undefined &&
        !(sections[page] as readonly string[]).includes(section)
      )
        return false;
      if (section !== undefined) state.selected[page] = section;
      state.page = page;
      return true;
    },
    cycle() {
      state.layoutStep = ((state.layoutStep + 1) % 4) as LayoutStep;
      if (state.layoutStep !== 0) state.restoreStep = state.layoutStep;
    },
    hideOrRestore() {
      if (state.layoutStep === 0) state.layoutStep = state.restoreStep;
      else {
        state.restoreStep = state.layoutStep;
        state.layoutStep = 0;
      }
    },
    toggleHistory() {
      state.page = "assistant";
      state.historyOpen = !state.historyOpen;
    },
    closeHistory() {
      state.historyOpen = false;
    },
    setSearch(open: boolean) {
      state.searchOpen = open;
    },
  };
}
export type Navigation = ReturnType<typeof createNavigation>;
