<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, nextTick } from "vue";
import { AppShell, NavigationList } from "@rss-mdm-agent/ui";
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
import Settings from "./settings/Settings.vue";
import TestUsers from "./settings/TestUsers.vue";
import { nativeHost } from "./settings/native";
import { createHostSettings } from "./settings/controller";
defineProps<{ assistantServices?: AssistantServices }>();
const users = ref<TestUser[]>([]),
  loading = ref(nativeTestMode),
  message = ref("");
const page = ref(nativeTestMode ? "settings" : "home"),
  attention = ref(0),
  mode = ref(nativeTestMode ? "本地测试模式" : "浏览器只读预览");
const content = ref<HTMLElement>();
const host = createHostSettings(nativeHost());
let polling: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    users.value = (await loadTestUsers()).users;
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
    attention.value = 0;
    page.value = "settings";
    await refresh();
  } catch (error) {
    message.value = selectionMessage(error);
  } finally {
    loading.value = false;
  }
}
async function navigate(id: string) {
  page.value = nativeTestMode && !currentUser.value ? "settings" : id;
  await nextTick();
  if (page.value === "settings")
    content.value?.querySelector<HTMLElement>(".settings h1")?.focus();
}
onMounted(() => {
  if (nativeTestMode) void refresh();
  if (host.available) {
    void host.refresh();
    polling = setInterval(() => void host.refresh(), 2000);
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
        <span class="mode-label">{{ mode }}</span>
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
            :current="currentUser"
            :native="nativeTestMode"
            :loading="loading"
            :message="message"
            @select="select"
        /></template>
      </Workspace>
      <Settings v-else :host="host"
        ><template #user
          ><TestUsers
            :users="users"
            :current="currentUser"
            :native="nativeTestMode"
            :loading="loading"
            :message="message"
            @select="select" /></template
      ></Settings>
    </div>
    <p v-if="loading" role="status">正在读取或切换测试用户…</p>
    <template #status
      ><div class="footer-note">
        <span>S1 测试服务 · 无系统副作用 · 独立测试批准</span
        ><span>AI 对话与设备执行分别核对</span>
      </div></template
    >
  </AppShell>
</template>
