<script setup lang="ts">
import { computed, ref } from "vue";
import type { AssistantController } from "../assistant/controller";
import { cancelNote } from "../assistant/command-presentation";
import ExecutionDetails from "../assistant/ExecutionDetails.vue";
const props = defineProps<{ controller: AssistantController }>();
const c = props.controller,
  s = c.state,
  view = c.view,
  clock = c.clock;
const taskId = ref("");
const retryLabel = new Map([
  ["same_command", "仅可核对并重试原命令，勿新建重复命令"],
  ["reconcile_first", "先恢复历史并核对，再决定后续操作"],
  ["never", "此命令不可重试"],
]);
const diagnostics = computed(() =>
  Object.values(view.value?.commands ?? {}).filter(
    (command) => command.failure,
  ),
);
const cancellations = computed(() =>
  Object.values(view.value?.commands ?? {}).filter(
    (command) => command.command.input.type === "cancel",
  ),
);
</script>
<template>
  <section class="session-diagnostics" aria-label="会话详情与诊断">
    <h3>会话详情与诊断</h3>
    <p>会话编号：{{ s.selected || "尚未创建" }}</p>

    <p>连接：{{ c.sessionConnection.value }}</p>
    <p v-if="s.cleanupError">连接清理未确认：{{ s.cleanupError }}</p>
    <div v-if="view" class="assistant-actions">
      <button
        :disabled="s.connection !== 'connected' || s.opening"
        @click="c.restore()"
      >
        重新读取历史
      </button>
      <button
        :disabled="!c.canResume.value || s.opening"
        @click="c.restore(true)"
      >
        恢复原模型上下文
      </button>
      <button :disabled="view.connection !== 'attached'" @click="c.detach">
        分离当前会话视图
      </button>
    </div>
    <section
      v-if="diagnostics.length"
      class="command-failures"
      aria-label="AI 命令诊断"
    >
      <h3>AI 命令诊断</h3>
      <template v-for="command in diagnostics" :key="command.command.commandId">
        <p v-if="command.failure" role="status">
          {{
            command.command.input.type === "prompt"
              ? "消息"
              : command.command.input.type === "cancel"
                ? "取消"
                : "回答"
          }}
          {{ command.command.commandId }}：{{ command.failure.code }}。{{
            retryLabel.get(command.failure.retry)
          }}；该诊断不证明模型已终止。
        </p>
      </template>
    </section>

    <p v-for="command in cancellations" :key="command.command.commandId">
      {{ cancelNote(command) }}
    </p>
    <details>
      <summary>按执行编号查询</summary>
      <form @submit.prevent="taskId.trim() && c.taskDetails(taskId.trim())">
        <label>执行请求编号<input v-model="taskId" maxlength="128" /></label
        ><button :disabled="!taskId.trim() || s.taskLoading">
          读取执行详情
        </button>
      </form>
      <p v-if="s.taskError" role="alert">{{ s.taskError }}</p>
      <ExecutionDetails v-if="s.task" :details="s.task" :now="clock" />
    </details>
  </section>
</template>
<style scoped>
.session-diagnostics {
  display: grid;
  gap: 12px;
}
h3,
p {
  margin: 0;
}
p {
  overflow-wrap: anywhere;
}
.assistant-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
form {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  gap: 8px;
  margin: 12px 0;
}
label {
  display: grid;
  gap: 8px;
  flex: 1;
  min-width: 180px;
}
</style>
