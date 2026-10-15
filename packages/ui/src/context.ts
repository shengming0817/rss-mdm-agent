import { inject, type InjectionKey, type Ref } from "vue";

export type UiPlatform = "windows" | "macos";
export type UiTheme = "system" | "light" | "dark";
export interface UiConfiguration {
  platform: UiPlatform;
  theme: UiTheme;
  reducedMotion: boolean;
  highContrast: boolean;
}
export interface UiContext {
  configuration: Readonly<UiConfiguration>;
  portal: Ref<HTMLElement | undefined>;
}
export const uiContext: InjectionKey<UiContext> = Symbol("ui-context");
export function useUiContext(): UiContext {
  const context = inject(uiContext);
  if (!context) throw new Error("UI controls require UiProvider");
  return context;
}
export interface UiFocusable { focus(): void }
export interface UiOption { value: string; label: string; disabled?: boolean }
export interface UiItem { id: string; label: string; disabled?: boolean }
export interface UiColumn { key: string; label: string; sortable?: boolean }
export interface UiRow extends UiItem { [key: string]: unknown }
export interface UiTreeNode extends UiItem {
  children?: UiTreeNode[];
  hasChildren?: boolean;
  loading?: boolean;
  error?: string;
}
