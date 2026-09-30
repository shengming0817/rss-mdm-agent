<script setup lang="ts">
import { ref, watch } from "vue";
import type { AssistantController } from "./controller";
import type {
  BackendTaskView,
  ExecutionTaskDetails,
} from "@rss-mdm-agent/execution-bindings/task-details";
const props = defineProps<{
  controller: AssistantController;
  operationId: string;
  recorded: boolean;
}>();
const emit = defineEmits<{ details: [value: ExecutionTaskDetails] }>();
const details = ref<BackendTaskView>();
const error = ref("");
const busy = ref(false);
let poll: () => Promise<void> = async () => {};
watch(
  () => [props.operationId, props.recorded, props.controller.state.selected],
  (_, __, cleanup) => {
    const owner = new AbortController();
    details.value = undefined;
    let loading = false;
    const refresh = async () => {
      if (loading || owner.signal.aborted) return;
      loading = true;
      try {
        const value = await props.controller.executionDetails(
          props.operationId,
          owner.signal,
        );
        if (!owner.signal.aborted) {
          details.value = value;
          error.value = "";
        }
      } catch {
        if (!owner.signal.aborted) error.value = "暂时无法读取原请求状态";
      } finally {
        loading = false;
      }
    };
    void refresh();
    poll = refresh;
    cleanup(() => {
      owner.abort();
      poll = async () => {};
    });
  },
  { immediate: true },
);
watch(
  () => Math.floor(props.controller.clock.value / 2000),
  () => {
    void poll();
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
  <section v-if="details" class="execution-activity" aria-label="设备操作">
    <template v-if="details.kind === 'execution'">
      <strong
        >设备操作 · {{ details.value.action.operation.resource.id }}</strong
      >
      <p v-if="details.value.status.phase === 'verified'">执行服务已核实结果</p>
      <p v-else>查看执行服务记录的状态与结果</p>
      <button @click="emit('details', details.value)">查看设备操作</button>
    </template>
    <template v-else>
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
    </template>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
