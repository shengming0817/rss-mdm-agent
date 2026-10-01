<script setup lang="ts">
import { useId } from "vue";
import { Sparkles } from "@rss-mdm-agent/ui";
import type { ResourceContext } from "./resource-context";
import type { AssistantController } from "./controller";
defineProps<{
  wide: boolean;
  selection: string;
  candidate: ResourceContext;
  controller: AssistantController;
  chosen: boolean;
  error: string;
  notice: string;
}>();
const emit = defineEmits<{
  close: [];
  choose: [target: string];
  main: [];
  "update:selection": [target: string];
}>();
const selectId = useId();
</script>
<template>
  <div class="context-panel" :class="{ 'context-aside': wide }">
    <header v-if="wide" class="context-heading">
      <h2><Sparkles :size="18" aria-hidden="true" />资源上下文 AI</h2>
      <button
        type="button"
        aria-label="关闭资源上下文 AI"
        @click="emit('close')"
      >
        关闭
      </button>
    </header>
    <p class="resource-path">{{ candidate.path.join(" → ") }}</p>
    <form
      v-if="!chosen"
      class="context-selection"
      @submit.prevent="selection && emit('choose', selection)"
    >
      <div class="context-intro">
        <h3>{{ candidate.path[2] }}</h3>
        <p>解释用途、核对版本与要求。选择会话后可检查本次发送的资源信息。</p>
      </div>
      <details>
        <summary>核对资源信息</summary>
        <pre>{{ candidate.text }}</pre>
      </details>
      <label :for="selectId">选择目标会话</label>
      <select
        :id="selectId"
        :value="selection"
        @change="
          emit('update:selection', ($event.target as HTMLSelectElement).value)
        "
      >
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
        :disabled="!selection || candidate.stale"
      >
        继续到所选会话
      </button>
    </form>
    <div v-else class="context-heading">
      <span>{{
        controller.context.value
          ? "资源附件待确认发送"
          : "资源附件已移除，按普通提问发送"
      }}</span
      ><button type="button" @click="emit('main')">在 AI 页面继续</button>
    </div>
    <p v-if="notice" role="status">{{ notice }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="context-host"><slot /></div>
  </div>
</template>
<style scoped>
.context-panel {
  min-width: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--rss-color-navigation);
  color: var(--rss-color-text);
}
.context-aside {
  border-left: 1px solid var(--rss-color-border);
  padding: 16px;
}
.context-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex: none;
  font-size: var(--rss-font-size-xs);
}
.context-heading h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: var(--rss-font-size-md);
}
.resource-path {
  flex: none;
  margin: 12px 0;
  padding: 8px 10px;
  border-radius: var(--rss-radius-sm);
  font-size: var(--rss-font-size-xs);
  color: var(--rss-color-text-muted);
  background: var(--rss-color-neutral-bg);
  overflow-wrap: anywhere;
}
.context-intro h3 {
  font-size: 22px;
  margin: 24px 0 12px;
}
.context-intro p {
  color: var(--rss-color-text-muted);
  font-size: var(--rss-font-size-sm);
  line-height: 1.7;
}
.context-selection {
  min-height: 0;
  overflow: auto;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 12px;
  padding: 0 4px 16px;
}
.context-selection label {
  font-size: var(--rss-font-size-sm);
  font-weight: 600;
}
.context-selection p {
  margin: 0;
  font-size: var(--rss-font-size-sm);
}
.context-selection pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 160px;
  overflow: auto;
  font-size: var(--rss-font-size-xs);
}
.context-selection select {
  width: 100%;
}
.context-host {
  flex: 1;
  min-height: 0;
}
.context-panel > [role] {
  flex: none;
  font-size: var(--rss-font-size-sm);
}
@media (max-height: 520px) {
  .resource-path {
    margin: 4px 0;
    padding: 4px 8px;
  }
  .context-heading button {
    min-height: 28px;
    padding: 4px 8px;
  }
  .context-intro h3 {
    margin: 8px 0;
  }
}
</style>
