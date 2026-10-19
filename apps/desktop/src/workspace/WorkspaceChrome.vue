<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { AppShell, PanelLeft, UiDialog } from "@rss-mdm-agent/ui";
import WindowControls from "./WindowControls.vue";
// ref: Vue v3.5.38 packages/runtime-core/src/componentSlots.ts — stable slot forwarding.
// The original AppShell and full sidebar slots stay mounted while the rail changes.
const props = defineProps<{
  navigationEnabled: boolean;
  navigationKey: string;
  contentMode: "conversation" | "page";
  designed: boolean;
  narrow: boolean;
}>();
const layout = ref<"rail" | "full" | "hidden">("rail");
const previous = ref<"rail" | "full">("rail");
const emit = defineEmits<{ search: [] }>();
const cycleButton = ref<HTMLButtonElement>();
async function closeNavigation() {
  layout.value = "rail";
  await nextTick();
  cycleButton.value?.focus();
}
function cycle() {
  layout.value =
    layout.value === "rail"
      ? "full"
      : layout.value === "full"
        ? "hidden"
        : "rail";
  if (layout.value !== "hidden") previous.value = layout.value;
}
function navigate() {
  if (props.narrow && layout.value === "full") layout.value = "rail";
}
function keys(event: KeyboardEvent) {
  if (
    (event.metaKey || event.ctrlKey) &&
    event.key.toLowerCase() === "k" &&
    !event.altKey &&
    props.navigationEnabled
  ) {
    event.preventDefault();
    emit("search");
    return;
  }
  if (
    (event.metaKey || event.ctrlKey) &&
    event.key.toLowerCase() === "b" &&
    !event.altKey &&
    props.navigationEnabled
  ) {
    event.preventDefault();
    if (layout.value === "hidden") layout.value = previous.value;
    else {
      previous.value = layout.value;
      layout.value = "hidden";
    }
  }
}
watch(
  () => props.narrow,
  (next) => {
    if (next && layout.value === "full") layout.value = "rail";
  },
);
</script>
<template>
  <div
    class="workspace-chrome"
    @keydown="keys"
    :data-layout="layout"
    :class="{ designed, 'navigation-disabled': !navigationEnabled }"
  >
    <aside
      v-if="navigationEnabled && layout === 'rail'"
      class="workspace-rail"
      aria-label="图标侧栏"
    >
      <div class="rail-scroll">
        <slot
          name="navigation"
          :navigate="navigate"
          :layout="layout"
          :inline-secondary="true"
        />
      </div>
      <slot name="navigation-footer" :navigate="navigate" :layout="layout" />
    </aside>
    <AppShell
      :navigation-enabled="navigationEnabled"
      :navigation-key="navigationKey"
      :content-mode="contentMode"
    >
      <template #navigation-brand><slot name="navigation-brand" /></template>
      <template #navigation="scope"
        ><slot
          name="navigation"
          :navigate="scope.navigate"
          :layout="layout"
          :inline-secondary="layout === 'full'"
      /></template>
      <template #conversations="scope"
        ><slot
          v-if="layout === 'full'"
          name="conversations"
          :navigate="scope.navigate"
      /></template>
      <template #navigation-footer="scope"
        ><slot
          name="navigation-footer"
          :navigate="scope.navigate"
          :layout="layout"
      /></template>
      <template #header>
        <div class="chrome-toolbar" data-tauri-drag-region>
          <div class="chrome-leading" data-tauri-drag-region>
            <span class="chrome-app-brand"><i>r.</i><strong>RSS</strong></span>
            <button
              v-if="navigationEnabled"
              ref="cycleButton"
              class="icon-button sidebar-cycle"
              type="button"
              :aria-label="
                layout === 'rail'
                  ? narrow
                    ? '打开主导航'
                    : '展开完整侧边栏'
                  : layout === 'full'
                    ? '隐藏侧边栏'
                    : '显示图标侧栏'
              "
              :title="'切换侧边栏 · ⌘/Ctrl+B 隐藏或恢复'"
              @click="cycle"
            >
              <PanelLeft :size="16" aria-hidden="true" />
            </button>
            <div
              v-if="navigationEnabled && layout === 'hidden'"
              class="hidden-navigation"
            >
              <slot
                name="navigation"
                :navigate="navigate"
                :layout="layout"
                :inline-secondary="false"
              />
            </div>
            <slot name="header" />
          </div>
          <div class="chrome-search"><slot name="search" /></div>
          <div class="chrome-actions" data-tauri-drag-region>
            <div
              v-if="navigationEnabled && layout === 'hidden'"
              class="hidden-account"
            >
              <slot name="navigation-footer" :navigate="navigate" />
            </div>
            <slot name="actions" />
            <WindowControls class="chrome-window-controls" />
          </div>
        </div>
        <div
          v-if="navigationEnabled && layout === 'hidden'"
          class="hidden-navigation hidden-secondary"
        >
          <slot name="secondary" />
        </div>
      </template>
      <slot />
      <template #status><slot name="status" /></template>
    </AppShell>
    <UiDialog
      v-if="navigationEnabled && narrow && layout === 'full'"
      :open="true"
      presentation="drawer"
      side="left"
      title="主导航"
      class="workspace-full-drawer"
      @update:open="closeNavigation"
    >
      <div class="navigation-brand"><slot name="navigation-brand" /></div>
      <div class="navigation-scroll">
        <slot
          name="navigation"
          :navigate="navigate"
          :layout="layout"
          :inline-secondary="true"
        />
      </div>
      <div class="navigation-footer">
        <slot name="navigation-footer" :navigate="navigate" :layout="layout" />
      </div>
    </UiDialog>
  </div>
</template>
<style scoped>
.workspace-chrome {
  position: relative;
  height: 100%;
  min-height: 0;
  display: flex;
  padding-top: 56px;
}
.workspace-chrome > :deep(.shell) {
  flex: 1;
  min-width: 0;
}
.workspace-chrome :deep(.navigation-trigger),
.workspace-chrome :deep(.navigation-brand) {
  display: none;
}
.workspace-chrome:not([data-layout="full"]) :deep(.navigation-panel) {
  display: none;
}
.workspace-chrome :deep(.shell .shell-content .shell-header) {
  position: absolute;
  inset: 0 0 auto;
  z-index: 10;
  height: 56px;
  min-height: 56px;
  padding: 0;
  border-bottom: 1px solid var(--rss-color-border);
  background: var(--rss-color-navigation);
}
:global([data-platform="macos"] .workspace-chrome) {
  --chrome-leading-space: 96px;
}
:global([data-platform="macos"] .chrome-window-controls) {
  display: none;
}
:global([data-platform="windows"] .workspace-chrome) {
  --chrome-trailing-space: 8px;
}
.chrome-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(240px, 400px) minmax(0, 1fr);
  gap: 16px;
  width: 100%;
  min-width: 0;
  height: 100%;
  user-select: none;
}
.chrome-leading,
.chrome-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  height: 100%;
}
.chrome-leading {
  padding-left: var(--chrome-leading-space, 16px);
}
.chrome-actions {
  justify-content: flex-end;
  padding-right: var(--chrome-trailing-space, 16px);
}
.chrome-search {
  display: flex;
  align-items: center;
  min-width: 0;
}
.sidebar-cycle {
  width: 30px;
  height: 30px;
  padding: 0;
  flex: none;
}
.chrome-leading :deep(.workspace-page-header) {
  overflow: hidden;
}
.workspace-chrome[data-layout="hidden"] .chrome-app-brand,
.workspace-chrome[data-layout="hidden"]
  .chrome-leading
  :deep(.workspace-page-header) {
  display: none;
}
.chrome-toolbar :deep(.window-controls) {
  margin: 0 -8px 0 4px;
}
.chrome-toolbar :deep(.hidden-account .account-avatar) {
  width: 26px;
  height: 26px;
}
.chrome-toolbar :deep(.hidden-account .account-avatar svg) {
  width: 16px;
  height: 16px;
}
.chrome-toolbar .hidden-navigation :deep(.workspace-primary-navigation > ol) {
  gap: 3px;
}
.chrome-toolbar
  .hidden-navigation
  :deep(.workspace-primary-navigation > ol > li > button) {
  min-height: 30px;
  height: 30px;
  padding: 6px;
  gap: 0;
  border-radius: 6px;
}
.chrome-toolbar
  .hidden-navigation
  :deep(.workspace-primary-navigation > ol > li > button > span:not(.ui-icon)) {
  display: none;
}
.chrome-toolbar .hidden-navigation :deep(.ui-icon),
.chrome-toolbar .hidden-navigation :deep(.ui-icon svg) {
  width: 16px;
  height: 16px;
  flex-basis: 16px;
}
.workspace-rail {
  display: flex;
  flex-direction: column;
  width: 68px;
  padding: 14px 8px 10px;
  flex: none;
  background: var(--rss-color-navigation);
  border-right: 1px solid var(--rss-color-border);
}
.rail-scroll {
  flex: 1;
  overflow: auto;
  min-height: 0;
  scrollbar-width: thin;
}
.workspace-chrome :deep(.navigation-panel) {
  background: var(--rss-color-navigation);
  border-right-color: var(--rss-color-border);
  padding-top: 14px;
}
.workspace-rail :deep(.workspace-account) {
  padding: 8px;
  margin: 0;
}
.workspace-rail :deep(.workspace-account > span:last-child) {
  display: none;
}
.hidden-account {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}
.hidden-account :deep(.workspace-account) {
  width: auto;
  margin: 0;
  padding: 4px;
}
.hidden-account :deep(.workspace-account > span:last-child) {
  display: none;
}
.hidden-navigation {
  min-width: 0;
  flex: 0 1 auto;
  overflow-x: auto;
  scrollbar-width: thin;
}
.hidden-navigation :deep(.ui-navigation ol) {
  display: flex;
  width: max-content;
}
.hidden-navigation :deep(.ui-navigation button) {
  padding: 8px 12px;
  white-space: nowrap;
}
.hidden-secondary {
  position: absolute;
  top: 56px;
  left: 0;
  right: 0;
  padding: 0 16px;
  overflow: auto;
  background: var(--rss-color-bg);
}
.workspace-chrome[data-layout="hidden"]:has(
    .hidden-secondary .module-navigation
  ) {
  padding-top: 110px;
}
.designed :deep(main.page),
.designed :deep(.shell-status) {
  padding: 0;
  background: var(--rss-color-bg);
}
.workspace-chrome :deep(.shell-status) {
  min-height: 26px;
  font-size: 9px;
  padding: 6px 30px;
  color: var(--rss-color-text-muted);
}
.chrome-app-brand {
  display: flex;
  align-items: center;
  gap: 9px;
  color: var(--rss-color-text-muted);
  font-size: 10px;
  letter-spacing: 0.5px;
  margin-right: 2px;
  flex: none;
}
.chrome-app-brand i {
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  background: #26333f;
  color: #fff;
  font-size: 17px;
  font-weight: 700;
  font-style: normal;
  letter-spacing: -2px;
}
.chrome-app-brand strong {
  font-weight: 600;
}
:global(.workspace-full-drawer) {
  background: var(--rss-color-navigation);
  --rss-drawer-width: 280px;
  --rss-drawer-padding: 16px;
}
:global(.workspace-full-drawer .navigation-brand) {
  display: none;
}
:global(.workspace-full-drawer .navigation-scroll) {
  overflow: auto;
  flex: 1;
  min-height: 0;
}
.navigation-footer {
  flex: none;
  padding-top: 12px;
}
@media (max-width: 1100px) {
  .chrome-app-brand {
    display: none;
  }
  .chrome-toolbar {
    grid-template-columns: minmax(0, 1fr) minmax(180px, 32vw) minmax(0, 1fr);
    gap: 10px;
  }
}
@media (max-width: 700px) {
  .workspace-rail {
    width: 60px;
    padding-inline: 4px;
  }
  .chrome-toolbar {
    grid-template-columns: minmax(0, 1fr) minmax(120px, 30vw) minmax(0, 1fr);
    gap: 6px;
  }
  .chrome-leading {
    gap: 3px;
    padding-left: var(--chrome-leading-space, 8px);
  }
  .chrome-leading :deep(.workspace-breadcrumb),
  .chrome-leading :deep(.workspace-page-header > h1),
  .chrome-leading :deep(.conversation-header h1) {
    display: none;
  }
  .chrome-leading :deep(.conversation-header) {
    gap: 0;
  }
  .chrome-actions {
    gap: 3px;
    padding-right: var(--chrome-trailing-space, 8px);
  }
  .hidden-account {
    gap: 2px;
  }
  .chrome-toolbar :deep(.hidden-account .conversation-shortcuts) {
    display: none;
  }
  .chrome-toolbar .hidden-navigation {
    flex: 1;
  }
}
</style>
