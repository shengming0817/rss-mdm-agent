<script setup lang="ts">
import { ref, watch } from "vue";
import ModalDrawer from "./ModalDrawer.vue";
const props = defineProps<{ navigationCollapsed?: boolean }>();
const navigationOpen = ref(false);
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
        class="navigation-trigger"
        aria-label="打开主导航"
        @click="navigationOpen = true"
      >
        ☰</button
      ><slot name="header" />
    </header>
    <div class="body">
      <aside v-if="!navigationCollapsed">
        <div class="navigation-scroll"><slot name="navigation" /></div>
        <div class="navigation-footer"><slot name="navigation-footer" /></div>
      </aside>
      <main><slot /></main>
    </div>
    <ModalDrawer
      v-if="navigationCollapsed && navigationOpen"
      label="主导航"
      @close="navigationOpen = false"
      ><div @click="navigationOpen = false">
        <slot name="navigation" /><slot name="navigation-footer" /></div
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
  flex: 0 0 200px;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px;
  border-right: 1px solid var(--color-border);
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
.navigation-trigger {
  padding: 8px 12px;
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
