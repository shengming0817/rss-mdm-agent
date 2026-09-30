<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { PanelLeft } from "@lucide/vue";
import ModalDrawer from "./ModalDrawer.vue";
const props = defineProps<{
  navigationEnabled: boolean;
  contentMode: "conversation" | "page";
  navigationKey: string;
}>();
const root = ref<HTMLElement>(),
  main = ref<HTMLElement>();
const compact = ref(false),
  navigationOpen = ref(false);
const navigationId = useId();
let observer: ResizeObserver | undefined;
async function navigate() {
  navigationOpen.value = false;
  await nextTick();
  const heading = [
    ...(main.value?.querySelectorAll<HTMLElement>("h1") ?? []),
  ].find((el) => el.getClientRects().length > 0);
  if (heading) {
    heading.tabIndex = -1;
    heading.focus();
  }
}
watch(() => props.navigationKey, navigate);
watch(compact, () => {
  navigationOpen.value = false;
});
onMounted(() => {
  if (!root.value) return;
  compact.value = root.value.getBoundingClientRect().width < 900;
  observer = new ResizeObserver((entries) => {
    compact.value = (entries[0]?.contentRect.width ?? 0) < 900;
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
  <div ref="root" class="rss-ui shell" :class="{ compact }">
    <header class="shell-header">
      <button
        v-if="navigationEnabled && compact"
        class="navigation-trigger icon-button"
        aria-label="打开主导航"
        :aria-expanded="navigationOpen"
        :aria-controls="navigationId"
        @click="navigationOpen = true"
      >
        <PanelLeft :size="20" aria-hidden="true" />
      </button>
      <slot name="header" />
    </header>
    <div class="shell-body">
      <aside
        v-if="navigationEnabled && !compact"
        class="navigation-panel"
        aria-label="工作区导航"
      >
        <div class="navigation-scroll">
          <div class="navigation-primary">
            <slot name="navigation" :navigate="navigate" />
          </div>
          <slot name="conversations" :navigate="navigate" />
        </div>
        <div class="navigation-footer">
          <slot name="navigation-footer" :navigate="navigate" />
        </div>
      </aside>
      <main ref="main" :class="contentMode"><slot /></main>
    </div>
    <ModalDrawer
      v-if="navigationEnabled && compact && navigationOpen"
      :id="navigationId"
      class="navigation-drawer"
      label="主导航"
      side="left"
      @close="navigationOpen = false"
    >
      <div class="navigation-scroll">
        <div class="navigation-primary">
          <slot name="navigation" :navigate="navigate" />
        </div>
        <slot name="conversations" :navigate="navigate" />
      </div>
      <div class="navigation-footer">
        <slot name="navigation-footer" :navigate="navigate" />
      </div>
    </ModalDrawer>
    <footer class="shell-status"><slot name="status" /></footer>
  </div>
</template>
<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--rss-color-bg);
}
.shell-header {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 48px;
  flex: none;
  padding: 6px 20px;
  background: var(--rss-color-surface);
  border-bottom: 1px solid var(--rss-color-border);
}
.shell-body {
  display: flex;
  flex: 1;
  min-height: 0;
}
.navigation-panel {
  flex: 0 0 var(--rss-navigation-width);
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px 12px;
  border-right: 1px solid var(--rss-color-border);
  background: var(--rss-color-bg);
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
.navigation-drawer {
  --rss-drawer-width: 280px;
  --rss-drawer-padding: 16px;
}
.navigation-drawer[open] {
  display: flex;
  flex-direction: column;
}
main {
  background: var(--rss-color-bg);
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 24px;
}
main.conversation {
  padding: 0;
  overflow: hidden;
}
.shell-status {
  flex: none;
  padding: 5px 16px;
  min-height: 26px;
  font-size: var(--rss-font-size-xs);
  color: var(--rss-color-text-muted);
  border-top: 1px solid var(--rss-color-border);
}
.compact .shell-header {
  padding: 6px 12px;
}
.compact main.page {
  padding: 16px;
}
</style>

<style scoped>
:global(.native-material) .shell {
  background: transparent;
}
:global(.native-material) .navigation-panel {
  background: var(--rss-color-native-navigation);
}
</style>
