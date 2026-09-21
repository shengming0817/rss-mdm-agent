<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { HistoryPreview } from "@rss-mdm-agent/ai-contract";
import { operationMessage, type AssistantController } from "./controller";
const props = defineProps<{ controller: AssistantController }>();
const c = props.controller;
const rows = computed(() => c.state.connections);
const busy = ref(false),
  error = ref("");
const historyMode = ref("none"),
  recent = ref(5),
  preview = ref<HistoryPreview>();
let historyRequest = 0;
const selected = computed(() =>
  c.connectionReady.value ? (c.view.value?.selectedConnectionId ?? "") : "",
);
const labels = new Map([
  ["codex", "Codex"],
  ["claude", "Claude"],
  ["deepseek", "DeepSeek"],
]);
async function run(action: () => Promise<void>) {
  if (busy.value || c.state.connection !== "connected") return;
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value = operationMessage(
      e && typeof e === "object" && "code" in e
        ? String(e.code)
        : "unavailable",
    );
  } finally {
    busy.value = false;
  }
}
watch(
  () => [c.state.selected, c.state.connection],
  () => {
    historyRequest++;
    preview.value = undefined;
    historyMode.value = "none";
  },
);
async function choose(id: string, fresh = false) {
  await run(async () => {
    if (!c.runtime.value || !c.state.selected) return;
    c.state.history.delete(c.state.selected);
    preview.value = undefined;
    historyMode.value = "none";
    await c.runtime.value.selectConnection(c.state.selected, id, fresh);
  });
}
async function history() {
  const request = ++historyRequest;
  c.state.history.delete(c.state.selected);
  preview.value = undefined;
  if (historyMode.value === "none") return;
  await run(async () => {
    const runtime = c.runtime.value,
      sessionId = c.state.selected,
      connectionId = selected.value;
    if (!runtime || !connectionId) return;
    const result = await runtime.previewHistory(
      sessionId,
      connectionId,
      historyMode.value === "recent" ? recent.value : undefined,
    );
    if (
      request === historyRequest &&
      runtime === c.runtime.value &&
      sessionId === c.state.selected &&
      connectionId === selected.value
    )
      preview.value = result;
  });
}
</script>
<template>
  <section class="connections" aria-label="会话连接">
    <fieldset :disabled="c.state.connection !== 'connected'">
      <template v-if="c.view.value">
        <p v-if="!c.connectionReady.value" role="status">
          当前会话需要选择可用连接。删除连接不会删除历史。
        </p>
        <label
          >本会话连接
          <select
            :value="selected"
            :disabled="busy"
            @change="choose(($event.target as HTMLSelectElement).value)"
          >
            <option value="" disabled>请选择连接</option>
            <option
              v-for="row in rows"
              :key="row.connectionId"
              :value="row.connectionId"
            >
              {{ row.name }} · {{ labels.get(row.provider) }}
            </option>
          </select></label
        >
        <button
          :disabled="busy || !selected || c.busy.value"
          @click="choose(selected, true)"
        >
          下次输入开始新上下文
        </button>
        <p>
          切换连接后，已接收的输入先按原连接完成；下一条输入使用新连接。对话记录始终保留。
        </p>
        <label
          >带入历史
          <select
            v-model="historyMode"
            :disabled="busy || c.busy.value"
            @change="history"
          >
            <option value="none">不带入（默认）</option>
            <option value="recent">最近 N 轮已完成对话</option>
            <option value="all">全部已完成对话（最多 64 KiB）</option>
          </select></label
        >
        <template v-if="historyMode === 'recent'"
          ><input
            v-model.number="recent"
            aria-label="历史轮数"
            type="number"
            min="1"
            max="10000"
          /><button :disabled="busy" @click="history">
            更新预览
          </button></template
        >
        <div v-if="preview" class="history-preview">
          <p>
            以下文本将随下一条输入发送给所选连接。不会带入工具输出、系统指令或原始附件。
          </p>
          <pre>{{ preview.text || "没有可带入的已完成对话" }}</pre>
          <button
            :disabled="busy"
            @click="c.state.history.set(c.state.selected, preview)"
          >
            确认带入这些文本</button
          ><span v-if="c.state.history.has(c.state.selected)"> 已确认</span>
        </div>
      </template>
    </fieldset>
    <p v-if="busy" role="status">正在处理连接…</p>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.connections {
  padding: 16px;
  border: 1px solid #d6e0df;
  border-radius: 10px;
  margin: 16px 0;
}
.connections p {
  color: #536765;
  font-size: 13px;
}
.connections button {
  margin: 4px;
}
.connection-form {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr));
  gap: 12px;
  margin: 16px 0;
}
.connection-form h3 {
  grid-column: 1/-1;
}
.connection-form label {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
input,
select {
  border: 1px solid #c6d6d3;
  border-radius: 6px;
  padding: 8px;
}
.history-preview pre {
  white-space: pre-wrap;
  max-height: 280px;
  overflow: auto;
  padding: 12px;
  background: #f3f6f5;
}
fieldset {
  border: 0;
  padding: 0;
  min-width: 0;
}
input,
select {
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
}
</style>
