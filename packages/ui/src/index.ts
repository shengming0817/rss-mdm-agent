export { default as AppShell } from "./components/AppShell.vue";
export { default as NavigationList } from "./components/NavigationList.vue";
export { default as SplitPane } from "./components/SplitPane.vue";
export { default as MessageStream } from "./components/MessageStream.vue";
export { default as MessageComposer } from "./components/MessageComposer.vue";
export { default as StatusList } from "./components/StatusList.vue";
export type { MessageItem, NavigationItem, StatusItem } from "./types";

export { default as ModalDrawer } from "./components/ModalDrawer.vue";

export {
  PopoverRoot as UiPopover,
  PopoverTrigger as UiPopoverTrigger,
  PopoverContent as UiPopoverContent,
  PopoverPortal as UiPopoverPortal,
  DropdownMenuRoot as UiMenu,
  DropdownMenuTrigger as UiMenuTrigger,
  DropdownMenuContent as UiMenuContent,
  DropdownMenuPortal as UiMenuPortal,
  DropdownMenuItem as UiMenuItem,
} from "reka-ui";
export {
  PanelLeft,
  MoreHorizontal,
  ChevronDown,
  X,
  Sparkles,
} from "@lucide/vue";
