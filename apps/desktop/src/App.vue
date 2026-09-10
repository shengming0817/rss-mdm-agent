<script setup lang="ts">
import { computed, ref } from "vue";
import {
  AppShell,
  NavigationList,
  SplitPane,
  MessageStream,
  MessageComposer,
  StatusList,
  type MessageItem,
  type StatusItem,
} from "@rss-mdm-agent/ui";
const activeId = ref("overview");
const draft = ref("");
const busy = ref(false);
const status = ref("就绪");
const items = ref<MessageItem[]>([
  {
    id: "welcome",
    kind: "assistant",
    text: "欢迎。这是桌面组件展示样本，所有交互只保存在当前窗口内存中。",
  },
  {
    id: "reason",
    kind: "reasoning",
    text: "固定推理样本：此处没有连接模型，也不会运行工具。",
  },
]);
const states = computed<StatusItem[]>(() => [
  {
    id: "sample",
    label: "样本状态",
    message: status.value,
    tone: busy.value ? "warning" : "neutral",
  },
]);
function send(text: string) {
  items.value.push({ id: `sample-${items.value.length}`, kind: "user", text });
  draft.value = "";
  status.value = "已记录样本输入";
}
function toggleBusy() {
  busy.value = !busy.value;
  status.value = busy.value ? "忙碌样本" : "就绪";
}
function cancel() {
  busy.value = false;
  status.value = "已取消样本等待";
}
</script>
<template>
  <AppShell>
    <template #header
      ><div class="heading">
        <strong>RSS MDM Agent</strong
        ><span class="sample-label">展示样本，无真实执行</span>
      </div></template
    >
    <template #navigation
      ><NavigationList
        :items="[
          { id: 'overview', label: '概览' },
          { id: 'conversation', label: '消息与输入' },
        ]"
        :active-id="activeId"
        @select="activeId = $event"
    /></template>
    <section v-if="activeId === 'overview'" class="overview">
      <p class="eyebrow">桌面基础组件</p>
      <h1>一个独立的桌面起点</h1>
      <p>布局、导航、消息、输入与状态组件已组合在当前窗口中。</p>
      <div class="cards">
        <article>
          <h2>原生窗口</h2>
          <p>Tauri 桌面壳承载 Vue 页面。</p>
        </article>
        <article>
          <h2>独立组件</h2>
          <p>同一套组件可以在浏览器中运行。</p>
        </article>
        <article>
          <h2>本地样本</h2>
          <p>无模型、无任务执行、无持久化。</p>
        </article>
      </div>
      <button type="button" @click="activeId = 'conversation'">
        查看消息与输入
      </button>
    </section>
    <section v-else class="conversation">
      <h1>消息与输入</h1>
      <SplitPane :initial-top-ratio="0.65" :min-ratio="0.2">
        <template #top><MessageStream :items="items" /></template>
        <template #bottom
          ><MessageComposer
            v-model="draft"
            :busy="busy"
            :can-cancel="busy"
            @submit="send"
            @cancel="cancel"
          /><button class="state-toggle" type="button" @click="toggleBusy">
            {{ busy ? "切换为就绪样本" : "切换为忙碌样本" }}
          </button></template
        >
      </SplitPane>
    </section>
    <template #status><StatusList :items="states" /></template>
  </AppShell>
</template>
<style scoped>
.heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}
.heading strong {
  font-size: 18px;
  letter-spacing: -0.3px;
}
.sample-label {
  font-size: 12px;
  color: var(--color-warn);
  background: var(--color-warn-bg);
  padding: 6px 10px;
  border-radius: 20px;
}
h1 {
  font-size: 28px;
  margin: 8px 0 20px;
}
h2 {
  font-size: 16px;
}
p {
  line-height: 1.7;
  color: var(--color-text-muted);
}
.overview {
  max-width: 900px;
  margin: 32px auto;
}
.eyebrow {
  color: var(--color-accent);
  font-weight: 600;
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 16px;
  margin: 32px 0;
}
article {
  padding: 20px;
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-surface);
}
button {
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  padding: 10px 16px;
  background: var(--color-surface);
}
.conversation {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.conversation h1 {
  font-size: 22px;
  flex: none;
}
.state-toggle {
  margin-top: 12px;
}
</style>
