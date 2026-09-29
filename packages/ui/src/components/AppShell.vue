<script setup lang="ts">
import { ref, watch, useId } from "vue";
import ModalDrawer from "./ModalDrawer.vue";
const props = defineProps<{ navigationCollapsed?: boolean }>();
const navigationOpen = ref(false);
const navigationId = useId();
watch(
  () => props.navigationCollapsed,
  () => {
    navigationOpen.value = false;
  },
);
</script>
<template>
  <div class="rss-ui shell" :class="{ focused: navigationCollapsed }">
    <header>
      <button
        v-if="navigationCollapsed"
        class="navigation-trigger icon-button"
        aria-label="打开主导航"
        :aria-expanded="navigationOpen"
        :aria-controls="navigationId"
        @click="navigationOpen = true"
      >
        <svg
          viewBox="0 0 24 24"
          width="20"
          height="20"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <path d="M4 6h16M4 12h16M4 18h16" />
        </svg></button
      ><slot name="header" />
    </header>
    <div class="body">
      <aside v-if="!navigationCollapsed" class="navigation-panel">
        <div class="navigation-scroll"><slot name="navigation" /></div>
        <div class="navigation-footer"><slot name="navigation-footer" /></div>
      </aside>
      <main><slot /></main>
    </div>
    <ModalDrawer
      v-if="navigationCollapsed && navigationOpen"
      :id="navigationId"
      class="navigation-drawer"
      label="主导航"
      side="left"
      @close="navigationOpen = false"
      ><div class="navigation-scroll" @click="navigationOpen = false">
        <slot name="navigation" />
      </div>
      <div class="navigation-footer" @click="navigationOpen = false">
        <slot name="navigation-footer" /></div
    ></ModalDrawer>
    <footer><slot name="status" /></footer>
  </div>
</template>
<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--color-bg);
}
header,
footer {
  flex: none;
  padding: 16px 24px;
  background: var(--color-surface);
}
header {
  display: flex;
  align-items: center;
  gap: 16px;
  border-bottom: 1px solid var(--color-border);
}
footer {
  border-top: 1px solid var(--color-border);
}
.body {
  display: flex;
  flex: 1;
  min-height: 0;
}
aside {
  flex: 0 0 var(--navigation-width);
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px;
  border-right: 1px solid var(--color-border);
  background: var(--color-bg);
}
.navigation-drawer {
  --drawer-width: var(--navigation-width);
  --drawer-padding: 16px;
}
.navigation-drawer[open] {
  display: flex;
  flex-direction: column;
}
.navigation-scroll {
  overflow: auto;
  min-height: 0;
  flex: 1;
}
.navigation-footer {
  flex: none;
  padding-top: 12px;
}
main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 24px;
}
.focused main {
  padding: 0;
  overflow: hidden;
}
header :deep(.brand) {
  flex: 1;
}
@media (max-width: 640px) {
  .body {
    flex-direction: column;
  }
  aside {
    flex: none;
    max-height: 140px;
    border-right: 0;
    border-bottom: 1px solid var(--color-border);
  }
  .navigation-scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  .navigation-footer {
    flex: none;
    padding-top: 12px;
  }
  main {
    padding: 16px;
  }
}
</style>
