<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import type { Controller } from "./controller";
import TaskDetail from "./TaskDetail.vue";
const props = defineProps<{ controller: Controller }>();
const c = props.controller;
const s = c.state;
const task = computed(() =>
  s.snapshot?.requests.find((t) => t.action.requestId === s.taskId),
);
let polling: ReturnType<typeof setInterval>;
onMounted(() => {
  void c.refresh();
  polling = setInterval(() => {
    if (!s.busy) void c.refresh();
  }, 2000);
});
onUnmounted(() => {
  clearInterval(polling);
  c.dispose();
});
</script>
<template>
  <div class="self-service">
    <h1>{{ s.page === "tasks" ? "设备任务" : "可用软件与任务" }}</h1>
    <p v-if="!c.interactive">请在桌面应用中连接本机执行服务。</p>
    <p v-if="s.error" class="error" role="alert">{{ s.error }}</p>
    <button :disabled="s.loading" @click="c.refresh">刷新</button>
    <section v-if="s.snapshot" aria-label="后台任务">
      <p v-if="!s.snapshot.available.length">当前没有等待操作的后台任务。</p>
      <article
        v-for="offer in s.snapshot.available"
        :key="offer.task + offer.attempt"
      >
        <h2>{{ offer.title }}</h2>
        <p>有效期：{{ new Date(offer.expiresAt * 1000).toLocaleString() }}</p>
        <button
          v-if="offer.userInitiated"
          :disabled="s.busy || s.uncertain"
          @click="c.select(offer)"
        >
          查看并确认
        </button>
        <p v-else>由后台派发，执行服务处理。</p>
      </article>
      <section v-if="s.item" aria-label="操作确认">
        <h2>确认执行 {{ s.item.title }}</h2>
        <p>将执行后台为此设备指定的任务。</p>
        <button :disabled="s.busy || s.uncertain" @click="c.confirm">
          确认并执行
        </button>
        <button :disabled="s.busy" @click="s.item = null">返回</button>
      </section>
      <section aria-label="执行记录">
        <button
          v-for="record in s.snapshot.requests"
          :key="record.action.requestId"
          @click="s.taskId = record.action.requestId"
        >
          {{ record.action.operation.resource.id }} · {{ record.status.phase }}
        </button>
        <button v-if="s.snapshot.next" :disabled="s.loading" @click="c.next">
          下一页
        </button>
      </section>
      <TaskDetail
        v-if="task"
        :task="task"
        :disabled="s.busy"
        @cancel="c.cancel(task.action.requestId)"
      />
    </section>
  </div>
</template>
