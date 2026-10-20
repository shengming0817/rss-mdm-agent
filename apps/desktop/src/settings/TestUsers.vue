<script setup lang="ts">
import { ref } from "vue";
import { UserRound } from "@rss-mdm-agent/ui";
import type { TestUser, UserContext } from "@rss-mdm-agent/ai-contract/wire";
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
    <header class="test-user-heading">
      <span><UserRound :size="20" aria-hidden="true" /></span>
      <div>
        <h3>本地测试工作区</h3>
        <p>测试模式 · 非登录 · 不代表企业身份认证</p>
      </div>
    </header>
    <small
      >切换用户会取消当前用户的模型请求并记录结果、清空未发送草稿；设备任务仍属于原用户并继续。</small
    >
    <span>{{
      current
        ? `当前用户：${current.user.displayName}`
        : "请先选择或创建测试用户"
    }}</span>
    <label class="test-user-label"
      >已有测试用户<select
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
      </select></label
    >
    <form @submit.prevent="select(name)">
      <label class="test-user-label"
        >新建或切换用户<input
          v-model="name"
          aria-label="测试用户名"
          placeholder="新建或切换测试用户"
          :disabled="loading || !native" /></label
      ><button :disabled="loading || !native || !name.trim()">进入</button>
    </form>
    <span v-if="loading" role="status">正在切换并清理旧视图…</span
    ><span v-if="message" role="alert">{{ message }}</span>
    <p v-if="!native" class="test-user-note">
      浏览器预览不能创建或切换测试用户。
    </p>
  </section>
</template>
<style scoped>
.test-users {
  display: grid;
  gap: 18px;
  padding: 24px;
  min-width: 0;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  font-size: 12px;
}
.test-user-heading {
  display: flex;
  align-items: center;
  gap: 12px;
}
.test-user-heading > span {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex: none;
  border-radius: 11px;
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text-muted);
}
h3 {
  margin: 0 0 6px;
  font-size: 15px;
  font-weight: 600;
}
p {
  margin: 0;
  color: var(--rss-color-text-muted);
  font-size: 11px;
  line-height: 1.8;
}
small {
  line-height: 1.8;
  color: var(--rss-color-text-muted);
  font-size: 11px;
}
.test-user-label {
  display: flex;
  flex-direction: column;
  gap: 9px;
  min-width: 0;
  font-size: 12px;
  font-weight: 500;
}
form {
  display: flex;
  align-items: end;
  gap: 10px;
}
form label {
  flex: 1;
}
input,
select {
  width: 100%;
  min-width: 0;
  max-width: 100%;
  min-height: 40px;
  padding: 10px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
  font: inherit;
  font-size: 13px;
}
button {
  min-height: 40px;
  padding: 9px 16px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
  font: inherit;
  cursor: pointer;
}
button:disabled,
input:disabled,
select:disabled {
  opacity: 0.55;
  cursor: default;
}
button:hover:enabled {
  background: var(--rss-color-neutral-bg);
}
button:focus-visible,
input:focus-visible,
select:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: 3px;
}
.test-user-note {
  border-top: 1px solid var(--rss-color-border);
  padding-top: 16px;
}
@media (max-width: 600px) {
  .test-users {
    padding: 18px;
  }
}
</style>
