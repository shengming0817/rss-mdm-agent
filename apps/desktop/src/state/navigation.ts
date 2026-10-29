import type { NavigationTarget } from "../workspace/page-navigation";
import { proxy } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type { LocalPreparation } from "../self-service/types";

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
    "category:效率办公",
    "category:创意设计",
    "category:网络工具",
    "category:开发工具",
    "category:系统工具",
    "installed",
    "updates",
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
    target: undefined as NavigationTarget | undefined,
    selected: Object.fromEntries(
      Object.entries(sections).map(([id, values]) => [id, values[0]]),
    ) as Record<PageId, string>,
    layoutStep: 0 as LayoutStep,
    restoreStep: 1 as Exclude<LayoutStep, 0>,
    historyOpen: false,
    searchOpen: false,
    policyOpen: false,
    preparation: null as { id: number; input: LocalPreparation } | null,
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
    locate(target: NavigationTarget) {
      state.target = target;
    },
    showPolicy() {
      state.policyOpen = true;
    },
    closePolicy() {
      state.policyOpen = false;
    },
    stage(input: LocalPreparation) {
      state.preparation = {
        id: (state.preparation?.id ?? 0) + 1,
        input: deepClone(input),
      };
      if (input.kind === "network") {
        state.page = "tools";
        state.selected.tools = "network";
      } else if (input.kind === "settings") {
        state.page = "system";
        state.selected.system =
          input.section === "lock" ? "power" : input.section;
      } else {
        state.page = "software";
        state.selected.software = "installed";
      }
    },
    clearPreparation() {
      state.preparation = null;
    },
    collapse() {
      state.layoutStep = 3;
      state.restoreStep = 3;
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
