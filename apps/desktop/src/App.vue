<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, nextTick, watch } from "vue";
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
import TestUsers from "./settings/TestUsers.vue";
import Account from "./settings/Account.vue";
import { nativeHost } from "./settings/native";
import { createHostSettings } from "./settings/controller";
defineProps<{ assistantServices?: AssistantServices }>();
const users = ref<TestUser[]>([]),
  loading = ref(nativeTestMode),
  message = ref(""),
  accountMessage = ref("");
const page = ref(nativeTestMode ? "settings" : "assistant");
const content = ref<HTMLElement>();
const host = createHostSettings(nativeHost());
let polling: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    users.value = (await loadTestUsers()).users;
    await refreshAccount();
    if (currentUser.value && page.value === "settings" && loading.value)
      page.value = "assistant";
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
    page.value = "settings";
    await refresh();
    page.value = "assistant";
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
  content.value
    ?.querySelector<HTMLElement>(
      page.value === "assistant" ? ".assistant h1" : ".settings h1",
    )
    ?.focus();
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
  <div ref="content" class="app-root">
    <Workspace
      :inert="loading ? true : undefined"
      :key="currentUser?.generation ?? 'anonymous'"
      :ready="!nativeTestMode || !!currentUser"
      :busy="loading"
      :assistant-services="assistantServices"
      :page="page"
      :host="host"
      @navigate="navigate"
    >
      <template #user>
        <TestUsers
          :users="users"
          :current="currentUser?.identity ? undefined : currentUser"
          :native="nativeTestMode"
          :loading="loading"
          :message="message"
          @select="select"
        />
        <Account
          :loading="loading"
          :message="accountMessage || accountNotice"
          @guest="accountAction(enterGuest)"
          @logout="accountAction(logoutAccount)"
          @login="
            (org, login) => accountAction(() => loginEnterprise(org, login))
          "
        />
      </template>
    </Workspace>
  </div>
  <p v-if="loading" class="account-progress" role="status">
    正在读取或切换账户…
  </p>
</template>
<style scoped>
.app-root {
  height: 100%;
  min-height: 0;
}
.account-progress {
  position: fixed;
  right: 16px;
  bottom: 4px;
  margin: 0;
  font: 12px var(--rss-font-sans);
  color: var(--rss-color-text-muted);
  background: var(--rss-color-bg);
}
</style>
