<script setup lang="ts">
import type { ExecutionTaskDetails } from "./types";
import RequestOrigin from "./RequestOrigin.vue";
defineProps<{ task: ExecutionTaskDetails; disabled: boolean }>();
const emit = defineEmits<{ cancel: [] }>();
</script>
<template>
  <section class="task-detail" aria-label="任务详情">
    <h2>{{ task.action.operation.resource.id }}</h2>
    <RequestOrigin :input="task.action" />
    <p>状态：{{ task.status.phase }}</p>
    <p v-if="task.status.process">
      根进程退出码：{{ task.status.process.exitCode ?? "尚未取得" }}
    </p>
    <p>效果核实：{{ task.status.assessment ?? "未知" }}</p>
    <p v-if="task.status.cancelRequested">已请求取消，等待执行服务核实终止。</p>
    <button
      :disabled="disabled || task.status.cancelRequested"
      @click="emit('cancel')"
    >
      请求取消原任务
    </button>
  </section>
</template>
