<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { PanelLeft } from "@lucide/vue";
import UiDialog from "./UiDialog.vue";
const props = defineProps<{
  navigationEnabled: boolean;
  contentMode: "conversation" | "page";
  navigationKey: string;
}>();
const root = ref<HTMLElement>();
const compact = ref(false),
  navigationOpen = ref(false),
  collapsed = ref(false);
const navigationWidth = ref(240);
const separator = ref<HTMLElement>();
const trigger = ref<HTMLButtonElement>();
const minWidth = 200,
  maxWidth = 360;
let drag: { pointer: number; x: number; width: number } | undefined;
function resize(value: number) {
  navigationWidth.value = Math.min(maxWidth, Math.max(minWidth, value));
}
function release() {
  if (drag && separator.value?.hasPointerCapture(drag.pointer))
    separator.value.releasePointerCapture(drag.pointer);
  drag = undefined;
}
function startResize(event: PointerEvent) {
  if (event.button !== 0 || drag) return;
  event.preventDefault();
  separator.value?.focus();
  separator.value?.setPointerCapture(event.pointerId);
  drag = {
    pointer: event.pointerId,
    x: event.clientX,
    width: navigationWidth.value,
  };
}
function moveResize(event: PointerEvent) {
  if (drag?.pointer === event.pointerId)
    resize(drag.width + event.clientX - drag.x);
}
function endResize(event: PointerEvent) {
  if (drag?.pointer === event.pointerId) release();
}
function resizeKey(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  resize(
    event.key === "Home"
      ? minWidth
      : event.key === "End"
        ? maxWidth
        : navigationWidth.value + (event.key === "ArrowLeft" ? -16 : 16),
  );
}
async function toggleNavigation() {
  release();
  if (compact.value) navigationOpen.value = !navigationOpen.value;
  else {
    collapsed.value = !collapsed.value;
    await nextTick();
    trigger.value?.focus();
  }
}
const navigationId = useId();
let observer: ResizeObserver | undefined;
async function navigate() {
  navigationOpen.value = false;
  await nextTick();
  const heading = [
    ...(root.value?.querySelectorAll<HTMLElement>("h1") ?? []),
  ].find((el) => el.getClientRects().length > 0);
  if (heading) {
    heading.tabIndex = -1;
    heading.focus();
  }
}
watch(() => props.navigationKey, navigate);
watch(compact, () => {
  release();
  navigationOpen.value = false;
});
watch(
  () => props.navigationEnabled,
  (enabled) => {
    if (!enabled) {
      release();
      navigationOpen.value = false;
    }
  },
);
onMounted(() => {
  if (!root.value) return;
  compact.value = root.value.getBoundingClientRect().width < 900;
  observer = new ResizeObserver((entries) => {
    compact.value = (entries[0]?.contentRect.width ?? 0) < 900;
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => {
  release();
  observer?.disconnect();
});
</script>
<template>
  <div ref="root" class="rss-ui shell" :class="{ compact }">
    <aside
      v-if="navigationEnabled && !compact"
      v-show="!collapsed"
      :id="navigationId"
      :style="{ flexBasis: `${navigationWidth}px` }"
      class="navigation-panel"
      aria-label="工作区导航"
    >
      <div class="navigation-brand"><slot name="navigation-brand" /></div>
      <div class="navigation-scroll">
        <div class="navigation-primary">
          <slot name="navigation" :navigate="navigate" />
        </div>
        <slot name="conversations" :navigate="navigate" />
      </div>
      <div class="navigation-footer">
        <slot name="navigation-footer" :navigate="navigate" />
      </div>
      <div
        ref="separator"
        class="navigation-resizer"
        role="separator"
        tabindex="0"
        aria-label="调整侧边栏宽度"
        aria-orientation="vertical"
        :aria-controls="navigationId"
        :aria-valuemin="minWidth"
        :aria-valuemax="maxWidth"
        :aria-valuenow="navigationWidth"
        :aria-valuetext="`${navigationWidth} 像素`"
        title="拖动调整宽度，双击恢复默认"
        @pointerdown="startResize"
        @pointermove="moveResize"
        @pointerup="endResize"
        @pointercancel="endResize"
        @lostpointercapture="endResize"
        @keydown="resizeKey"
        @dblclick="resize(240)"
      />
    </aside>
    <div class="shell-content">
      <header class="shell-header">
        <button
          v-if="navigationEnabled"
          ref="trigger"
          type="button"
          class="navigation-trigger icon-button"
          :aria-label="
            compact ? '打开主导航' : collapsed ? '展开侧边栏' : '收起侧边栏'
          "
          :title="
            compact ? '打开主导航' : collapsed ? '展开侧边栏' : '收起侧边栏'
          "
          :aria-expanded="compact ? navigationOpen : !collapsed"
          :aria-controls="navigationId"
          @click="toggleNavigation"
        >
          <PanelLeft :size="20" aria-hidden="true" />
        </button>
        <slot name="header" />
      </header>
      <main :class="contentMode"><slot /></main>
      <footer class="shell-status"><slot name="status" /></footer>
    </div>
    <UiDialog
      :open="true"
      presentation="drawer"
      v-if="navigationEnabled && compact && navigationOpen"
      :id="navigationId"
      class="navigation-drawer"
      title="主导航"
      side="left"
      @update:open="navigationOpen = false"
    >
      <div class="navigation-brand"><slot name="navigation-brand" /></div>
      <div class="navigation-scroll">
        <div class="navigation-primary">
          <slot name="navigation" :navigate="navigate" />
        </div>
        <slot name="conversations" :navigate="navigate" />
      </div>
      <div class="navigation-footer">
        <slot name="navigation-footer" :navigate="navigate" />
      </div>
    </UiDialog>
  </div>
</template>
<style scoped>
.shell {
  display: flex;
  height: 100%;
  min-height: 0;
  background: var(--rss-color-bg);
}
.shell-content {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}
.shell-header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 52px;
  flex: none;
  padding: 8px 24px;
  background: var(--rss-color-bg);
}
.navigation-panel {
  position: relative;
  flex: 0 0 var(--rss-navigation-width);
  min-width: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 24px 12px 12px;
  border-right: 1px solid var(--rss-color-border);
  background: var(--rss-color-navigation);
}
.navigation-resizer {
  position: absolute;
  inset-block: 0;
  right: -4px;
  width: 8px;
  z-index: 2;
  cursor: col-resize;
  touch-action: none;
  user-select: none;
}
.navigation-resizer::after {
  content: "";
  position: absolute;
  inset-block: 0;
  left: 3px;
  width: 2px;
  background: transparent;
}
.navigation-resizer:hover::after,
.navigation-resizer:focus-visible::after {
  background: var(--rss-color-accent);
}
.navigation-resizer:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: -2px;
}
.navigation-brand {
  flex: none;
  padding: 0 12px 24px;
}
.navigation-primary,
.navigation-footer {
  flex: none;
}
.navigation-scroll {
  overflow: auto;
  min-height: 0;
  flex: 1;
}
.navigation-footer {
  padding-top: 12px;
}
:global(.navigation-drawer) {
  --rss-drawer-width: 280px;
  --rss-drawer-padding: 16px;
  background: var(--rss-color-navigation);
}
main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 24px;
  background: var(--rss-color-bg);
}
main.conversation {
  padding: 0;
  overflow: hidden;
}
.shell-status {
  flex: none;
  padding: 6px 24px;
  min-height: 26px;
  font-size: var(--rss-font-size-xs);
  color: var(--rss-color-text-muted);
}
.compact .shell-header {
  padding: 6px 12px;
}
.compact main.page {
  padding: 16px;
}
:global(.native-material) .shell {
  background: transparent;
}
:global(.native-material) .navigation-panel {
  background: var(--rss-color-native-navigation);
}
</style>
