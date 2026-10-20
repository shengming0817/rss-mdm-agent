<script setup lang="ts">
import { UiSearchInput } from "@rss-mdm-agent/ui";
import ConversationList from "../assistant/ConversationList.vue";
import type { AssistantController } from "../assistant/controller";
defineProps<{ controller: AssistantController; query: string }>();
const emit = defineEmits<{ "update:query": [value: string]; select: [] }>();
</script>
<template>
  <div class="conversation-history-content">
    <UiSearchInput
      :model-value="query"
      aria-label="搜索已加载会话"
      placeholder="搜索历史会话"
      @update:model-value="emit('update:query', $event)"
    />
    <ConversationList
      :controller="controller"
      :show-create="false"
      :query="query"
      @select="emit('select')"
    />
  </div>
</template>
<style scoped>
.conversation-history-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  flex: 1;
}
.conversation-history-content :deep(.conversation-list) {
  overflow: auto;
  min-height: 0;
  flex: 1;
  scrollbar-width: thin;
}
.conversation-history-content :deep(.conversation-list h2) {
  margin: 10px 12px;
  font-size: 12px;
  font-weight: 500;
}
.conversation-history-content :deep(.conversation-list button) {
  font-size: 13px;
  min-height: 38px;
}
.conversation-history-content :deep(.conversation-list p) {
  color: var(--rss-color-text-muted);
  font-size: 12px;
  padding: 0 12px;
}
</style>
