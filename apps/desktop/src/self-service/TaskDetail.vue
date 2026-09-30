<script setup lang="ts">
import type { ExecutionTaskDetails } from "./types";
import ExecutionDetails from "../assistant/ExecutionDetails.vue";
defineProps<{ task: ExecutionTaskDetails; disabled: boolean }>();
const emit = defineEmits<{ cancel: [] }>();
</script>
<template>
  <section class="task-detail" aria-label="任务详情">
    <h2>{{ task.action.operation.resource.id }}</h2>
    <ExecutionDetails :details="task" :now="Date.now()" />
    <p>请查询原请求核对检测与终止证据；结果未知时不要重复派发。</p>
    <button
      :disabled="disabled || task.status.cancelRequested"
      @click="emit('cancel')"
    >
      请求取消原任务
    </button>
  </section>
</template>
