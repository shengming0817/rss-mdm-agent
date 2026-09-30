<script setup lang="ts">
import { ref, useId } from "vue";
import type { ResourceContext } from "./resource-context";
import type { AssistantController } from "./controller";
defineProps<{
  wide: boolean;
  candidate: ResourceContext;
  controller: AssistantController;
  chosen: boolean;
  error: string;
  notice: string;
}>();
const emit = defineEmits<{ close: []; choose: [target: string]; main: [] }>();
const target = ref("");
const selectId = useId();
</script>
<template>
  <div class="context-panel" :class="{ 'context-aside': wide }">
    <header v-if="wide" class="context-heading">
      <h2>资源上下文 AI</h2>
      <button
        type="button"
        aria-label="关闭资源上下文 AI"
        @click="emit('close')"
      >
        关闭
      </button>
    </header>
    <form
      v-if="!chosen"
      class="context-selection"
      @submit.prevent="target && emit('choose', target)"
    >
      <p class="resource-path">{{ candidate.path.join(" → ") }}</p>
      <details>
        <summary>核对资源信息</summary>
        <pre>{{ candidate.text }}</pre>
      </details>
      <label :for="selectId">选择目标会话</label>
      <select :id="selectId" v-model="target">
        <option value="">请选择会话</option>
        <option value="new">新建会话（首次发送时创建）</option>
        <option
          v-for="session in controller.sessions.value"
          :key="session.namespace.sessionId"
          :value="session.namespace.sessionId"
          :disabled="session.status === 'retired'"
        >
          {{ session.title }} · {{ session.namespace.sessionId }}
        </option>
      </select>
      <p v-if="controller.state.listing" role="status">正在读取会话…</p>
      <p v-if="controller.state.listError" role="alert">
        暂时无法读取会话。
        <button
          type="button"
          data-action="retry-sessions"
          :disabled="controller.state.listing"
          @click="controller.list(!!controller.state.next)"
        >
          重试
        </button>
      </p>
      <button
        v-else-if="controller.state.next"
        type="button"
        data-action="load-sessions"
        :disabled="controller.state.listing"
        @click="controller.list()"
      >
        加载更多会话
      </button>
      <p>不会自动发送；选择后可核对或移除资源附件。</p>
      <button
        type="submit"
        data-action="choose-conversation"
        :disabled="!target || candidate.stale"
      >
        继续到所选会话
      </button>
    </form>
    <div v-else class="context-heading">
      <span>正在使用工作区同一会话</span
      ><button type="button" @click="emit('main')">在 AI 页面继续</button>
    </div>
    <p v-if="notice" role="status">{{ notice }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="context-host"><slot /></div>
  </div>
</template>
<style scoped>
.context-panel {
  height: 100%;
  --rss-drawer-width: 600px;
  --rss-drawer-padding: 12px;
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
}
.context-panel,
.context-aside {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.context-aside {
  flex: 1;
  border-left: 1px solid var(--rss-color-border);
  padding: 12px;
  box-sizing: border-box;
}
.context-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex: none;
}
h2 {
  font-size: 16px;
  margin: 0;
}
.context-selection {
  overflow: auto;
}
.resource-path {
  overflow-wrap: anywhere;
}
select {
  display: block;
  width: 100%;
  margin: 8px 0;
}
pre {
  font-size: 12px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 180px;
  overflow: auto;
}
.context-host {
  min-height: 0;
  flex: 1;
}
.context-host :deep(.assistant) {
  height: 100%;
}
</style>
