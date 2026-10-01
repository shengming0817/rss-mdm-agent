<script setup lang="ts">
import {
  Sparkles,
  House,
  Package,
  Terminal,
  Activity,
  Monitor,
  Settings,
} from "@lucide/vue";
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
          <Sparkles
            v-if="item.icon === 'assistant'"
            :size="18"
            aria-hidden="true"
          />
          <House
            v-else-if="item.icon === 'home'"
            :size="18"
            aria-hidden="true"
          />
          <Package
            v-else-if="item.icon === 'software'"
            :size="18"
            aria-hidden="true"
          />
          <Terminal
            v-else-if="item.icon === 'tools'"
            :size="18"
            aria-hidden="true"
          />
          <Activity
            v-else-if="item.icon === 'tasks'"
            :size="18"
            aria-hidden="true"
          />
          <Monitor
            v-else-if="item.icon === 'device'"
            :size="18"
            aria-hidden="true"
          />
          <Settings
            v-else-if="item.icon === 'settings'"
            :size="18"
            aria-hidden="true"
          />
          <span class="navigation-label">{{ item.label }}</span>
          <span
            v-if="item.badge"
            class="navigation-badge"
            :aria-label="`待回应 ${item.badge}`"
            >{{ item.badge }}</span
          >
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
  gap: 4px;
}
button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  text-align: left;
  padding: 8px 12px;
  border: 0;
  border-radius: var(--rss-radius-md);
  background: transparent;
  overflow-wrap: anywhere;
}
button:hover {
  background: var(--rss-color-surface-hover);
}
button[aria-current] {
  background: var(--rss-color-accent-bg);
  color: var(--rss-color-accent);
  font-weight: 600;
}
</style>

<style scoped>
.navigation-label {
  flex: 1;
  min-width: 0;
}
.navigation-badge {
  padding: 1px 6px;
  border-radius: var(--rss-radius-sm);
  background: var(--rss-color-accent-bg);
  font-size: var(--rss-font-size-xs);
}
button:not([aria-current]) {
  color: var(--rss-color-text-muted);
}
</style>
