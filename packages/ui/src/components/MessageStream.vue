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
          <span class="user-label">你 / You</span>
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
  margin-top: var(--rss-space-4);
  display: flex;
  flex-direction: column;
  gap: var(--rss-space-4);
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
  max-width: 80%;
  padding: var(--rss-space-2) var(--rss-space-4);
  border-radius: var(--rss-space-3);
  background: var(--rss-color-neutral-bg);
  border: 1px solid var(--rss-color-border-strong);
}
.user-label {
  display: block;
  font-size: var(--rss-font-size-sm);
  color: var(--rss-color-text-muted);
  margin-bottom: var(--rss-space-2);
}
.user-bubble .text {
  line-height: 1.5;
}
</style>
