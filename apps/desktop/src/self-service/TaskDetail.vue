<script setup lang="ts">
import type { RequestView } from "./types";
import ActionSummary from "./ActionSummary.vue";
defineProps<{ task: RequestView; disabled: boolean; now: number }>();
const emit = defineEmits<{ confirm: []; cancel: [] }>();
</script>
<template>
  <section class="task-detail" aria-label="任务详情">
    <h2>{{ task.action.title }}</h2>
    <p class="notice" role="status">{{ task.message }}</p>
    <ActionSummary :input="task.action" />
    <p v-if="task.status === 'approval'">
      等待策略授权；本人确认不能替代批准，此处没有批准入口。
    </p>
    <section v-if="task.confirmation" class="interaction-card">
      <h3>动作确认</h3>
      <p>{{ task.confirmation.message }}</p>
      <p
        v-if="
          task.confirmation.status === 'pending' &&
          task.confirmation.expiresAtUnixMs <= now
        "
      >
        按本机时间已过期；服务端在确认时核验
      </p>
      <div
        v-if="
          task.status === 'confirmation' &&
          task.confirmation.status === 'pending' &&
          task.confirmation.expiresAtUnixMs > now
        "
        class="actions"
      >
        <button :disabled="disabled" @click="emit('confirm')">
          确认并执行
        </button>
        <button :disabled="disabled" @click="emit('cancel')">取消执行</button>
      </div>
    </section>
    <button
      v-if="['waiting', 'approval', 'unknownEffect'].includes(task.status)"
      :disabled="disabled"
      @click="emit('cancel')"
    >
      请求取消原任务
    </button>
  </section>
</template>
