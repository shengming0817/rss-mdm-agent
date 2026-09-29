<script setup lang="ts">
import { ref, watch } from "vue";
import type { AssistantController } from "./controller";
import type { ExecutionTaskDetails } from "@rss-mdm-agent/execution-bindings/task-details";
const props = defineProps<{
  controller: AssistantController;
  operationId: string;
  recorded: boolean;
}>();
const emit = defineEmits<{ details: [value: ExecutionTaskDetails] }>();
const details = ref<ExecutionTaskDetails>();
watch(
  () => [props.operationId, props.recorded, props.controller.state.selected],
  async (_, __, cleanup) => {
    const owner = new AbortController();
    cleanup(() => owner.abort());
    details.value = undefined;
    try {
      const value = await props.controller.executionDetails(
        props.operationId,
        owner.signal,
      );
      if (!owner.signal.aborted) details.value = value;
    } catch {
      /* An intent or model reply alone cannot establish an execution card. */
    }
  },
  { immediate: true },
);
</script>
<template>
  <section v-if="details" class="execution-activity" aria-label="设备操作">
    <strong>设备操作 · {{ details.action.operation.resource.id }}</strong>
    <p v-if="details.status.phase === 'confirmationRequired'">
      需要确认具体动作
    </p>
    <p v-else-if="details.status.phase === 'verified'">执行服务已核实结果</p>
    <p v-else>查看执行服务记录的状态与结果</p>
    <button @click="emit('details', details)">查看设备操作</button>
  </section>
</template>
