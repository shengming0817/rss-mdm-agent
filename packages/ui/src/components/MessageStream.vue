<script setup lang="ts">
// Text interpolation intentionally keeps untrusted content inert.
import MarkdownContent from "./MarkdownContent";
import type { MessageItem } from "../types";

defineEmits<{ copy: [text: string] }>();

withDefaults(
  defineProps<{ items: readonly MessageItem[]; announce?: boolean }>(),
  { announce: true },
);
</script>

<template>
  <div
    class="rss-ui stream"
    :role="announce ? 'log' : undefined"
    aria-label="消息"
    :aria-live="announce ? 'polite' : undefined"
    aria-relevant="additions text"
  >
    <template v-for="item in items" :key="item.id">
      <details v-if="item.kind === 'reasoning'" class="reasoning">
        <summary>推理过程</summary>
        <pre class="text">{{ item.text }}</pre>
      </details>

      <div v-else-if="item.kind === 'user'" class="user-row">
        <div class="user-bubble">
          <span class="sr-only">你</span>
          <pre class="text">{{ item.text }}</pre>
        </div>
      </div>
      <MarkdownContent
        v-else
        class="message"
        :text="item.text"
        :stable="item.stable"
        @copy="$emit('copy', $event)"
      />
    </template>
  </div>
</template>

<style scoped>
.stream {
  margin-top: 0;
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font: inherit;
}
.message {
  line-height: 1.75;
}
.reasoning {
  border-left: 2px solid var(--rss-color-border-strong);
  padding-left: var(--rss-space-4);
}
.reasoning summary {
  cursor: pointer;
  color: var(--rss-color-text-muted);
  font-size: var(--rss-font-size-sm);
}
.reasoning .text {
  margin-top: var(--rss-space-2);
  color: var(--rss-color-text-muted);
}
.user-row {
  display: flex;
  justify-content: flex-end;
}
.user-bubble {
  max-width: 86%;
  padding: 10px 15px;
  border-radius: 16px 16px 4px 16px;
  font-size: 14px;
  background: var(--rss-color-neutral-bg);
}
.user-bubble .text {
  line-height: 1.75;
}
</style>
