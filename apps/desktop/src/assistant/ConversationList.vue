<script setup lang="ts">
import { MessageSquare, Plus } from "@rss-mdm-agent/ui";
import type { AssistantController } from "./controller";
const props = defineProps<{ controller: AssistantController }>();
const emit = defineEmits<{ select: [] }>();
function select(id?: string) {
  if (id) void props.controller.select(id);
  else props.controller.create();
  emit("select");
}
</script>
<template>
  <nav aria-label="最近对话" class="conversation-list">
    <button class="new-conversation" @click="select()">
      <Plus :size="18" aria-hidden="true" />新对话
    </button>
    <h2>最近对话</h2>
    <p v-if="controller.state.listError" role="alert">
      暂时无法读取会话。<button
        :disabled="controller.state.listing"
        @click="controller.list(false)"
      >
        重试
      </button>
    </p>
    <ul>
      <li
        v-for="session in controller.sessions.value"
        :key="session.namespace.sessionId"
      >
        <button
          :aria-current="
            controller.state.selected === session.namespace.sessionId
              ? 'page'
              : undefined
          "
          @click="select(session.namespace.sessionId)"
        >
          <MessageSquare :size="15" aria-hidden="true" /><span
            class="conversation-title"
            >{{ session.title }}</span
          ><small v-if="session.status === 'retired'">已结束</small>
        </button>
      </li>
    </ul>
    <button
      v-if="controller.state.next"
      :disabled="controller.state.listing"
      @click="controller.list()"
    >
      加载更多会话
    </button>
  </nav>
</template>
<style scoped>
ul {
  list-style: none;
  margin: 0;
  padding: 0;
}
li {
  margin: 6px 0;
}
.conversation-list button {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  text-align: left;
  min-width: 0;
  padding: 7px 12px;
  border: 1px solid transparent;
  border-radius: var(--rss-radius-sm);
  background: transparent;
  color: var(--rss-color-text);
  cursor: pointer;
}
.conversation-list button[aria-current] {
  background: var(--rss-color-accent-bg);
  border-color: transparent;
}
.conversation-list .new-conversation {
  border-color: transparent;
  font-weight: 500;
}
h2 {
  font-size: var(--rss-font-size-sm);
  color: var(--rss-color-text-muted);
  margin: 20px 12px 8px;
}
.conversation-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}
svg {
  flex: none;
}
</style>
