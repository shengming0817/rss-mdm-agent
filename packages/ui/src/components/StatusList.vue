<script setup lang="ts">
import type { StatusItem } from "../types";
defineProps<{ items: readonly StatusItem[] }>();
</script>
<template>
  <ul class="rss-ui statuses" aria-label="状态">
    <li v-for="item in items" :key="item.id" :class="item.tone">
      <span class="dot" aria-hidden="true"></span
      ><strong>{{ item.label }}</strong
      ><span>{{ item.message }}</span>
    </li>
  </ul>
</template>
<style scoped>
.statuses {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 24px;
  list-style: none;
  margin: 0;
  padding: 0;
}
li {
  display: flex;
  align-items: baseline;
  gap: 8px;
  overflow-wrap: anywhere;
  min-width: 0;
}
.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-text-muted);
}
.success .dot {
  background: var(--color-success);
}
.warning .dot {
  background: var(--color-warn);
}
.danger .dot {
  background: var(--color-danger);
}
strong {
  font-weight: 600;
}
li > span:last-child {
  color: var(--color-text-muted);
}
</style>
