<script setup lang="ts">
import type { NavigationItem } from "../types";
withDefaults(
  defineProps<{
    items: readonly NavigationItem[];
    activeId?: string;
    label?: string;
  }>(),
  { label: "导航" },
);
const emit = defineEmits<{ select: [id: string] }>();
</script>
<template>
  <nav class="rss-ui" :aria-label="label">
    <ul>
      <li v-for="item in items" :key="item.id">
        <button
          type="button"
          :disabled="item.disabled"
          :aria-current="item.id === activeId ? 'page' : undefined"
          @click="!item.disabled && emit('select', item.id)"
        >
          {{ item.label }}
        </button>
      </li>
    </ul>
  </nav>
</template>
<style scoped>
ul {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 6px;
}
button {
  width: 100%;
  text-align: left;
  padding: 12px;
  border: 0;
  border-radius: var(--radius-md);
  background: transparent;
  overflow-wrap: anywhere;
}
button:hover {
  background: var(--color-surface-hover);
}
button[aria-current] {
  background: var(--color-accent-bg);
  color: var(--color-accent);
  font-weight: 600;
}
</style>
