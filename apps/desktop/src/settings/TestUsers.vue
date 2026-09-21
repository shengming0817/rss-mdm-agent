<script setup lang="ts">
import { ref } from "vue";
import type { TestUser, UserContext } from "@rss-mdm-agent/ai-contract";
defineProps<{
  users: TestUser[];
  current?: UserContext;
  loading: boolean;
  message: string;
  native: boolean;
}>();
const emit = defineEmits<{ select: [name: string] }>();
const name = ref("");
function select(value: string) {
  emit("select", value);
  name.value = "";
}
</script>
<template>
  <section class="test-users" aria-label="测试用户">
    <strong>测试模式</strong>
    <small
      >切换用户会取消当前用户的模型请求并记录结果、清空未发送草稿；设备任务仍属于原用户并继续。</small
    >
    <span>{{
      current
        ? `当前用户：${current.user.displayName}`
        : "请先选择或创建测试用户"
    }}</span>
    <select
      aria-label="已有测试用户"
      :disabled="loading || !native"
      :value="current?.user.userId ?? ''"
      @change="
        select(
          users.find(
            (user) =>
              user.userId === ($event.target as HTMLSelectElement).value,
          )?.displayName ?? '',
        )
      "
    >
      <option value="" disabled>选择测试用户</option>
      <option v-for="user in users" :key="user.userId" :value="user.userId">
        {{ user.displayName }}
      </option>
    </select>
    <form @submit.prevent="select(name)">
      <input
        v-model="name"
        aria-label="测试用户名"
        placeholder="新建或切换测试用户"
        :disabled="loading || !native"
      /><button :disabled="loading || !native || !name.trim()">进入</button>
    </form>
    <span v-if="loading" role="status">正在切换并清理旧视图…</span
    ><span v-if="message" role="alert">{{ message }}</span>
  </section>
  <p v-if="!native">浏览器预览不能创建或切换测试用户。</p>
</template>
<style scoped>
.test-users {
  display: grid;
  gap: 12px;
}
form {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
input,
select {
  min-width: 0;
  max-width: 100%;
  padding: 8px;
}
small {
  line-height: 1.5;
}
</style>
