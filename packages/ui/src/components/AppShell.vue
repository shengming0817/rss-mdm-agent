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
    ...(root.value?.querySelectorAll<HTMLElement>("h1") ?? []),
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
    <aside
      v-if="navigationEnabled && !compact"
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
    </aside>
    <div class="shell-content">
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
      <main ref="main" :class="contentMode"><slot /></main>
      <footer class="shell-status"><slot name="status" /></footer>
    </div>
    <ModalDrawer
      v-if="navigationEnabled && compact && navigationOpen"
      :id="navigationId"
      class="navigation-drawer"
      label="主导航"
      side="left"
      @close="navigationOpen = false"
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
    </ModalDrawer>
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
  flex: 0 0 var(--rss-navigation-width);
  min-width: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 24px 12px 12px;
  border-right: 1px solid var(--rss-color-border);
  background: var(--rss-color-navigation);
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
.navigation-drawer {
  --rss-drawer-width: 280px;
  --rss-drawer-padding: 16px;
  background: var(--rss-color-navigation);
}
.navigation-drawer[open] {
  display: flex;
  flex-direction: column;
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
