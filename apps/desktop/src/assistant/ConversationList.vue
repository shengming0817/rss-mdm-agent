<script setup lang="ts">
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
    <button class="new-conversation" @click="select()">新对话</button>
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
          {{ session.title
          }}<small v-if="session.status === 'retired'">已结束</small>
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
button {
  width: 100%;
  text-align: left;
  overflow-wrap: anywhere;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text);
  cursor: pointer;
}
button[aria-current] {
  background: var(--color-accent-bg);
  border-color: var(--color-accent);
}
.new-conversation {
  border-color: var(--color-border-strong);
}
h2 {
  font-size: 13px;
  color: var(--color-text-muted);
  margin: 24px 12px 8px;
}
</style>
