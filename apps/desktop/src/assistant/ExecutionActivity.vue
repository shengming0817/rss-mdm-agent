<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { ModalDrawer } from "@rss-mdm-agent/ui";
import ExecutionDetails from "./ExecutionDetails.vue";
import { taskPresentation } from "./execution-presentation";
import type { AssistantController } from "./controller";
import type { ExecutionTaskDetails } from "@rss-mdm-agent/execution-bindings/task-details";
const props = defineProps<{
  controller: AssistantController;
  operationId: string;
  recorded: boolean;
  visible: boolean;
  now: number;
}>();
defineEmits<{ tasks: [] }>();
const details = ref<ExecutionTaskDetails>(),
  error = ref(""),
  loading = ref(false),
  detailsOpen = ref(false);
const presentation = computed(
  () => details.value && taskPresentation(details.value),
);
let owner: AbortController | undefined;
async function refresh() {
  if (!props.visible || loading.value) return;
  const request = new AbortController();
  owner = request;
  loading.value = true;
  error.value = "";
  try {
    const value = await props.controller.executionDetails(
      props.operationId,
      request.signal,
    );
    if (!request.signal.aborted) details.value = value;
  } catch {
    if (!request.signal.aborted)
      error.value = "无法读取授权执行记录，请核对原请求。";
  } finally {
    if (owner === request) {
      loading.value = false;
      owner = undefined;
    }
  }
}
watch(
  () => [
    props.operationId,
    props.recorded,
    props.controller.state.selected,
    props.controller.view.value?.generation,
    props.visible,
  ],
  (next, previous, cleanup) => {
    owner?.abort();
    owner = undefined;
    loading.value = false;
    cleanup(() => owner?.abort());
    if (
      !previous ||
      next[0] !== previous[0] ||
      next[2] !== previous[2] ||
      next[3] !== previous[3]
    ) {
      details.value = undefined;
      error.value = "";
      detailsOpen.value = false;
    }
    if (!props.visible) {
      detailsOpen.value = false;
      return;
    }
    void refresh();
  },
  { immediate: true },
);
onBeforeUnmount(() => owner?.abort());
</script>
<template>
  <section
    v-if="details && presentation"
    class="execution-activity"
    aria-label="设备操作"
  >
    <div class="execution-heading">
      <strong>设备操作 · {{ details.action.operation.resource.id }}</strong
      ><span
        class="task-status"
        :class="{ attention: presentation.attention }"
        >{{ presentation.label }}</span
      >
    </div>
    <small>{{
      details.status.mode === "test"
        ? "S1 测试执行器 · 无真实设备变更"
        : "执行服务记录"
    }}</small>
    <p>{{ presentation.stage }}</p>
    <p v-if="presentation.cancel" class="muted">{{ presentation.cancel }}</p>
    <p v-if="error" role="alert">{{ error }} 当前显示上次已读取记录。</p>
    <div class="assistant-actions">
      <button @click="detailsOpen = true">查看设备操作</button
      ><button :disabled="loading" @click="refresh">
        {{ loading ? "正在读取…" : "刷新状态" }}</button
      ><button v-if="presentation.attention" @click="$emit('tasks')">
        前往任务处理
      </button>
    </div>
    <ModalDrawer
      v-if="visible && detailsOpen"
      label="设备操作详情"
      side="right"
      @close="detailsOpen = false"
    >
      <ExecutionDetails :details="details" :now="now" />
      <p v-if="error" role="alert">{{ error }} 当前显示上次已读取记录。</p>
      <button :disabled="loading" @click="refresh">刷新执行状态</button>
      <button
        v-if="details.status.phase === 'confirmationRequired'"
        @click="
          detailsOpen = false;
          $emit('tasks');
        "
      >
        前往任务确认动作
      </button>
    </ModalDrawer>
  </section>
  <p v-else-if="error" class="execution-read-error" role="status">
    {{ error }}
    <button :disabled="loading" @click="refresh">重新读取原请求</button>
  </p>
</template>
