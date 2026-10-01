<script setup lang="ts">
import {
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  nextTick,
  watch,
} from "vue";
import Workspace from "./Workspace.vue";
import { createAppearance } from "./appearance";
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
import type { TestUser, UserContext } from "@rss-mdm-agent/ai-contract";
import TestUsers from "./settings/TestUsers.vue";
import Account from "./settings/Account.vue";
import { nativeHost } from "./settings/native";
import { createHostSettings } from "./settings/controller";
import { assertFixtureAssembly, type FixtureEnvironment } from "./assembly";
import type { SelfServicePort } from "./self-service/types";
const props = defineProps<{
  assistantServices?: AssistantServices;
  selfServicePort?: SelfServicePort;
  environment?: FixtureEnvironment;
}>();
assertFixtureAssembly(props);
const selectedUser = props.environment
  ? shallowRef<UserContext>()
  : currentUser;
const accountEnabled = !props.environment && nativeTestMode;
const users = ref<TestUser[]>([]),
  loading = ref(!!props.environment || accountEnabled),
  message = ref(""),
  accountMessage = ref("");
const page = ref(accountEnabled ? "settings" : "assistant");
const content = ref<HTMLElement>();
const host = createHostSettings(
  props.environment ? props.environment.host : nativeHost(),
);
const appearance = createAppearance();
let polling: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    users.value = (await loadTestUsers()).users;
    await refreshAccount();
    if (selectedUser.value && page.value === "settings" && loading.value)
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
  // The page header is moved after its host ref has been bound.
  await nextTick();
  const title =
    page.value === "assistant"
      ? ".assistant-header-host h1"
      : ".workspace-page-header > h1";
  content.value?.querySelector<HTMLElement>(title)?.focus();
}
async function navigate(id: string) {
  page.value = accountEnabled && !selectedUser.value ? "settings" : id;
  if (page.value === "settings") await focusSettings();
}
watch(selectedUser, (next, previous) => {
  if (accountEnabled && previous && !next) {
    page.value = "settings";
    void focusSettings();
  }
});
onMounted(() => {
  if (props.environment)
    void props.environment.identity.read().then((value) => {
      selectedUser.value = value;
      loading.value = false;
    });
  if (accountEnabled) void refresh();
  if (host.available) {
    void host.refresh();
  }
  void appearance.refresh();
  polling = setInterval(() => {
    void appearance.refresh();
    if (host.available) void host.refresh();
    if (
      !props.environment &&
      !loading.value &&
      selectedUser.value?.identity?.mode === "enterprise"
    )
      void refreshAccount();
  }, 2000);
});
onBeforeUnmount(() => {
  if (polling) clearInterval(polling);
  host.dispose();
  appearance.dispose();
});
</script>
<template>
  <div
    ref="content"
    class="app-root"
    :class="{
      'native-material': appearance.state.materialEnabled,
      'reduced-motion': appearance.state.reducedMotion,
      'high-contrast': appearance.state.highContrast,
    }"
  >
    <Workspace
      :inert="loading ? true : undefined"
      :key="selectedUser?.generation ?? 'anonymous'"
      :ready="environment ? !!selectedUser : !accountEnabled || !!selectedUser"
      :busy="loading"
      :assistant-services="assistantServices"
      :self-service-port="selfServicePort"
      :page="page"
      :host="host"
      :service-port="environment?.service"
      :fixture="!!environment"
      @navigate="navigate"
    >
      <template #account-summary>
        <strong>{{ selectedUser?.user.displayName ?? "账户与连接" }}</strong>
        <small>{{
          selectedUser?.identity?.mode === "enterprise"
            ? "企业工作区"
            : selectedUser?.identity?.mode === "guest"
              ? "不登录使用"
              : selectedUser
                ? "测试用户 · 非企业认证"
                : "选择使用身份"
        }}</small>
      </template>
      <template #user>
        <p v-if="environment">开发 fixture 身份，不访问生产账户。</p>
        <template v-else>
          <TestUsers
            :users="users"
            :current="selectedUser?.identity ? undefined : selectedUser"
            :native="accountEnabled"
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
      </template>
    </Workspace>
  </div>
  <aside v-if="environment" class="fixture-controls" aria-label="开发 fixture">
    <strong>开发 fixture · 无真实设备执行</strong
    ><slot name="fixture-controls" />
  </aside>
  <p v-if="loading" class="account-progress" role="status">
    正在读取或切换账户…
  </p>
</template>
<style scoped>
.app-root {
  height: 100%;
  min-height: 0;
}
.app-root :deep(.workspace-account small) {
  display: block;
  margin-top: 3px;
  color: var(--rss-color-text-muted);
  font-size: var(--rss-font-size-xs);
}
.fixture-controls {
  position: fixed;
  top: 4px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 150;
  padding: 4px 8px;
  border-radius: 6px;
  font: 11px var(--rss-font-sans);
  background: var(--rss-color-bg);
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
