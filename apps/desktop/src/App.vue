<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, nextTick, watch } from "vue";
import { AppShell, NavigationList } from "@rss-mdm-agent/ui";
import Workspace from "./Workspace.vue";
import type { AssistantServices } from "./assistant/controller";
import {
  currentUser,
  accountNotice,
  accountErrorMessage,
  nativeTestMode,
  loadTestUsers,
  selectTestUser,
  selectionMessage,
  enterGuest,
  loginEnterprise,
  logoutAccount,
  refreshAccount,
} from "./test-users";
import type { TestUser } from "@rss-mdm-agent/ai-contract";
import Settings from "./settings/Settings.vue";
import TestUsers from "./settings/TestUsers.vue";
import Account from "./settings/Account.vue";
import { nativeHost } from "./settings/native";
import { createHostSettings } from "./settings/controller";
defineProps<{ assistantServices?: AssistantServices }>();
const users = ref<TestUser[]>([]),
  loading = ref(nativeTestMode),
  message = ref(""),
  accountMessage = ref("");
const page = ref(nativeTestMode ? "settings" : "home"),
  attention = ref(0),
  mode = ref(nativeTestMode ? "本地测试模式" : "浏览器只读预览");
const content = ref<HTMLElement>();
const host = createHostSettings(nativeHost());
let polling: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    users.value = (await loadTestUsers()).users;
    await refreshAccount();
  } catch {
    message.value = "无法读取测试用户记录";
  } finally {
    loading.value = false;
  }
}
async function select(name: string) {
  if (loading.value || !name.trim()) return;
  loading.value = true;
  message.value = "";
  try {
    await selectTestUser(name);
    accountNotice.value = "";
    accountMessage.value = "";
    attention.value = 0;
    page.value = "settings";
    await refresh();
  } catch (error) {
    message.value = selectionMessage(error);
  } finally {
    loading.value = false;
    await focusSettings();
  }
}
async function accountAction(action: () => Promise<unknown>) {
  if (loading.value) return;
  loading.value = true;
  accountMessage.value = "";
  accountNotice.value = "";
  try {
    await action();
    attention.value = 0;
    page.value = "settings";
  } catch (error) {
    accountMessage.value = accountErrorMessage(error);
  } finally {
    loading.value = false;
    await focusSettings();
  }
}
async function focusSettings() {
  await nextTick();
  content.value?.querySelector<HTMLElement>(".settings h1")?.focus();
}
async function navigate(id: string) {
  page.value = nativeTestMode && !currentUser.value ? "settings" : id;
  if (page.value === "settings") await focusSettings();
}
watch(currentUser, (next, previous) => {
  if (nativeTestMode && previous && !next) {
    page.value = "settings";
    void focusSettings();
  }
});
onMounted(() => {
  if (nativeTestMode) void refresh();
  if (host.available) {
    void host.refresh();
    polling = setInterval(() => {
      void host.refresh();
      if (!loading.value && currentUser.value?.identity?.mode === "enterprise")
        void refreshAccount();
    }, 2000);
  }
});
onBeforeUnmount(() => {
  if (polling) clearInterval(polling);
  host.dispose();
});
</script>
<template>
  <AppShell>
    <template #header
      ><div class="brand">
        <div>
          <span class="eyebrow">RSS / WORKSPACE</span
          ><strong>自助服务中心</strong>
        </div>
        <span class="mode-label">{{
          currentUser?.identity?.mode === "enterprise"
            ? "企业账户 · 本地 AI"
            : currentUser?.identity?.mode === "guest"
              ? "不登录 · 本地访客"
              : mode
        }}</span>
      </div></template
    >
    <template #navigation
      ><NavigationList
        :items="[
          { id: 'home', label: '首页' },
          { id: 'software', label: '软件中心' },
          { id: 'tools', label: '工具中心' },
          { id: 'tasks', label: '请求与任务' },
          {
            id: 'assistant',
            label: attention ? `AI 助手（待回应 ${attention}）` : 'AI 助手',
          },
          { id: 'help', label: '设备与帮助' },
        ]"
        :active-id="page"
        @select="navigate"
    /></template>
    <template #navigation-footer
      ><NavigationList
        :items="[{ id: 'settings', label: '设置' }]"
        :active-id="page"
        @select="navigate"
    /></template>
    <div ref="content" :inert="loading ? true : undefined">
      <Workspace
        v-if="!nativeTestMode || currentUser"
        :key="currentUser?.generation ?? 'browser-preview'"
        :assistant-services="assistantServices"
        :page="page"
        :host="host"
        @navigate="navigate"
        @attention="attention = $event"
        @mode="mode = $event"
      >
        <template #user
          ><TestUsers
            :users="users"
            :current="currentUser?.identity ? undefined : currentUser"
            :native="nativeTestMode"
            :loading="loading"
            :message="message"
            @select="select" /><Account
            :loading="loading"
            :message="accountMessage || accountNotice"
            @guest="accountAction(enterGuest)"
            @logout="accountAction(logoutAccount)"
            @login="
              (org, login) => accountAction(() => loginEnterprise(org, login))
            "
        /></template>
      </Workspace>
      <Settings v-else :host="host"
        ><template #user
          ><TestUsers
            :users="users"
            :current="undefined"
            :native="nativeTestMode"
            :loading="loading"
            :message="message"
            @select="select" /><Account
            :loading="loading"
            :message="accountMessage || accountNotice"
            @guest="accountAction(enterGuest)"
            @logout="accountAction(logoutAccount)"
            @login="
              (org, login) => accountAction(() => loginEnterprise(org, login))
            " /></template
      ></Settings>
    </div>
    <p v-if="loading" role="status">正在读取或切换账户…</p>
    <template #status
      ><div class="footer-note">
        <span>S1 测试服务 · 无系统副作用 · 独立测试批准</span
        ><span>AI 对话与设备执行分别核对</span>
      </div></template
    >
  </AppShell>
</template>
