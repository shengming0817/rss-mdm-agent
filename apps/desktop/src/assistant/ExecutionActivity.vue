<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { ModalDrawer } from "@rss-mdm-agent/ui";
import ExecutionDetails from "./ExecutionDetails.vue";
import { taskPresentation } from "./execution-presentation";
import type { AssistantController } from "./controller";
import type { BackendTaskView } from "@rss-mdm-agent/execution-bindings/task-details";
const props = defineProps<{
  controller: AssistantController;
  operationId: string;
  recorded: boolean;
  visible: boolean;
  now: number;
}>();
defineEmits<{ tasks: [] }>();
const details = ref<BackendTaskView>(),
  error = ref(""),
  loading = ref(false),
  detailsOpen = ref(false);
const presentation = computed(
  () =>
    details.value?.kind === "execution" &&
    taskPresentation(details.value.value),
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
const busy = ref(false);
watch(
  () => Math.floor(props.controller.clock.value / 2000),
  () => {
    void refresh();
  },
);
async function act(confirm: boolean) {
  const view = details.value;
  if (view?.kind !== "pending" || busy.value) return;
  busy.value = true;
  try {
    if (confirm) await props.controller.confirmPreparation(view.value.offer);
    else await props.controller.cancelPreparation(view.value.offer.request);
  } catch {
    error.value = "操作回执未确认，请查询原请求状态";
  } finally {
    busy.value = false;
  }
}

function operationLabel(value: string) {
  switch (value) {
    case "install":
      return "安装";
    case "uninstall":
      return "卸载";
    case "detect":
      return "检测";
    default:
      return "未知操作";
  }
}
function stateLabel(value: string) {
  switch (value) {
    case "proposed":
      return "等待本人确认";
    case "selected":
      return "已确认";
    case "submitting":
      return "准备执行";
    case "failed":
      return "准备失败；没有创建新尝试";
    case "cancelled":
      return "已撤销";
    default:
      return "状态未知";
  }
}
function failureLabel(value: string) {
  switch (value) {
    case "preparationFailed":
      return "本机条件或后台 Start 未满足";
    case "interrupted":
      return "服务中断，未重新执行";
    case "expired":
      return "原任务已过期";
    case "revoked":
      return "后台授权已撤销";
    default:
      return "状态未知";
  }
}
</script>
<template>
  <section
    v-if="details?.kind === 'execution' && presentation"
    class="execution-activity"
    aria-label="设备操作"
  >
    <div class="execution-heading">
      <strong
        >设备操作 · {{ details.value.action.operation.resource.id }}</strong
      ><span
        class="task-status"
        :class="{ attention: presentation.attention }"
        >{{ presentation.label }}</span
      >
    </div>
    <small>{{
      details.value.status.mode === "test"
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
      <ExecutionDetails :details="details.value" :now="now" />
      <p v-if="error" role="alert">{{ error }} 当前显示上次已读取记录。</p>
      <button :disabled="loading" @click="refresh">刷新执行状态</button>
      <button
        v-if="details.value.status.phase === 'confirmationRequired'"
        @click="
          detailsOpen = false;
          $emit('tasks');
        "
      >
        前往任务确认动作
      </button>
    </ModalDrawer>
  </section>
  <section
    v-else-if="details?.kind === 'pending'"
    class="execution-activity"
    aria-label="设备操作"
  >
    <strong>设备操作 · {{ details.value.offer.title }}</strong>
    <template v-if="details.value.offer.summary.kind === 'software'">
      <p>操作：{{ operationLabel(details.value.offer.summary.intent) }}</p>
      <ol>
        <li
          v-for="(step, index) in details.value.offer.summary.steps"
          :key="index"
        >
          {{ step.package }} {{ step.version }} ·
          {{ step.identity === "system" ? "系统账号" : "当前用户" }}
        </li>
      </ol>
    </template>
    <p>
      {{ stateLabel(details.value.state) }}
    </p>
    <p v-if="details.value.failure">
      {{ failureLabel(details.value.failure) }}
    </p>
    <button
      v-if="details.value.state === 'proposed'"
      :disabled="busy"
      @click="act(true)"
    >
      确认上述操作
    </button>
    <button
      v-if="!['failed', 'cancelled'].includes(details.value.state)"
      :disabled="busy"
      @click="act(false)"
    >
      撤销请求
    </button>

    <p v-if="error" role="alert">{{ error }}</p>
    <button :disabled="loading" @click="refresh">刷新状态</button>
  </section>
  <p v-else-if="error" class="execution-read-error" role="status">
    {{ error }}
    <button :disabled="loading" @click="refresh">重新读取原请求</button>
  </p>
</template>
