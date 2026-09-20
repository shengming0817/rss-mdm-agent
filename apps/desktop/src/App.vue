<script setup lang="ts">
import { onMounted, ref } from "vue";
import Workspace from "./Workspace.vue";
import type { AssistantServices } from "./assistant/controller";
import {
  currentUser,
  nativeTestMode,
  loadTestUsers,
  selectTestUser,
  selectionMessage,
} from "./test-users";
import type { TestUser } from "@rss-mdm-agent/ai-contract";
defineProps<{ assistantServices?: AssistantServices }>();
const users = ref<TestUser[]>([]),
  name = ref(""),
  loading = ref(nativeTestMode),
  message = ref("");
async function refresh() {
  try {
    const page = await loadTestUsers();
    users.value = page.users;
  } catch {
    message.value = "无法读取测试用户记录";
  } finally {
    loading.value = false;
  }
}
async function select(value = name.value) {
  if (loading.value || !value.trim()) return;
  loading.value = true;
  message.value = "";
  try {
    await selectTestUser(value);
    name.value = "";
    await refresh();
  } catch (error) {
    message.value = selectionMessage(error);
  } finally {
    loading.value = false;
  }
}
onMounted(() => {
  if (nativeTestMode) void refresh();
});
</script>
<template>
  <section v-if="nativeTestMode" class="test-users" aria-label="测试用户">
    <strong>测试模式</strong>
    <small
      >切换用户会取消当前用户的模型请求并记录结果、清空未发送草稿；设备任务仍属于原用户并继续。</small
    >
    <span>{{
      currentUser
        ? `当前用户：${currentUser.user.displayName}`
        : "请先选择或创建测试用户"
    }}</span>
    <select
      aria-label="已有测试用户"
      :disabled="loading"
      :value="currentUser?.user.userId ?? ''"
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
    <form @submit.prevent="select()">
      <input
        v-model="name"
        aria-label="测试用户名"
        placeholder="新建或切换测试用户"
        :disabled="loading"
      /><button :disabled="loading || !name.trim()">进入</button>
    </form>
    <span v-if="loading" role="status">正在切换并清理旧视图…</span
    ><span v-if="message" role="alert">{{ message }}</span>
  </section>
  <div :inert="loading ? true : undefined">
    <Workspace
      v-if="!nativeTestMode || currentUser"
      :key="currentUser?.generation ?? 'browser-preview'"
      :assistant-services="assistantServices"
    />
  </div>
</template>
<style scoped>
.test-users {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px 24px;
  background: #eef5f1;
  border-bottom: 1px solid #d4e4dc;
}
.test-users form {
  display: flex;
  gap: 8px;
}
.test-users input,
.test-users select {
  padding: 7px 10px;
  border: 1px solid #b9cfc2;
  border-radius: 6px;
}
</style>
