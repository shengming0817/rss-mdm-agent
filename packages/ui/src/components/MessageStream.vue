<script setup lang="ts">
// Text interpolation intentionally keeps untrusted content inert.
import type { MessageItem } from "../types";

defineProps<{ items: readonly MessageItem[] }>();
</script>

<template>
  <div class="rss-ui stream">
    <template v-for="item in items" :key="item.id">
      <details v-if="item.kind === 'reasoning'" class="reasoning">
        <summary>推理过程</summary>
        <pre class="text">{{ item.text }}</pre>
      </details>

      <div v-else-if="item.kind === 'user'" class="user-row">
        <div class="user-bubble">
          <span class="user-label">你 / You</span>
          <pre class="text">{{ item.text }}</pre>
        </div>
      </div>
      <pre v-else class="message text">{{ item.text }}</pre>
    </template>
  </div>
</template>

<style scoped>
.stream {
  margin-top: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}
.text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font: inherit;
}
.message {
  line-height: 1.5;
}
.reasoning {
  border-left: 2px solid var(--color-border-strong);
  padding-left: var(--space-4);
}
.reasoning summary {
  cursor: pointer;
  color: var(--color-text-muted);
  font-size: var(--font-size-sm);
}
.reasoning .text {
  margin-top: var(--space-2);
  color: var(--color-text-muted);
}
.user-row {
  display: flex;
  justify-content: flex-end;
}
.user-bubble {
  max-width: 80%;
  padding: var(--space-2) var(--space-4);
  border-radius: var(--space-3);
  background: var(--color-bg-subtle);
  border: 1px solid var(--color-border-strong);
}
.user-label {
  display: block;
  font-size: var(--font-size-sm);
  color: var(--color-text-muted);
  margin-bottom: var(--space-2);
}
.user-bubble .text {
  line-height: 1.5;
}
</style>
