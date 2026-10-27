<script setup lang="ts">
import {
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  nextTick,
  watch,
} from "vue";
import { workspaceIdentity } from "./workspace/identity";
import Workspace from "./Workspace.vue";
import { UiProvider } from "@rss-mdm-agent/ui";
import { createAppearance, desktopPlatform } from "./appearance";
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
  recoverAccount,
} from "./test-users";
import type { TestUser, UserContext } from "@rss-mdm-agent/ai-contract/wire";
import TestUsers from "./settings/TestUsers.vue";
import Account from "./settings/Account.vue";
import LoginScreen from "./account/LoginScreen.vue";
import type { EnterpriseLogin } from "./account/types";
import { nativeHost, nativeService } from "./settings/native";
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
const page = ref("home");
const loginOpen = ref(false);
const registrationPending = ref(false);
let registrationEpoch = 0;
const service = props.environment ? undefined : nativeService();
const servicePort = service && {
  ...service,
  async requestRegistration(input?: EnterpriseLogin) {
    registrationEpoch += 1;
    registrationPending.value = true;
    try {
      return await service.requestRegistration(input);
    } catch (error) {
      if (!selectedUser.value)
        accountMessage.value = accountErrorMessage(error);
      throw error;
    } finally {
      registrationPending.value = false;
    }
  },
};
const content = ref<HTMLElement>();
const host = createHostSettings(
  props.environment ? props.environment.host : nativeHost(),
);
const appearance = createAppearance();
let polling: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  try {
    users.value = (await loadTestUsers()).users;
    if (selectedUser.value) loginOpen.value = false;
    await refreshAccount();
    if (!selectedUser.value) await enterGuest();
    if (selectedUser.value) loginOpen.value = false;
    if (selectedUser.value && page.value === "settings" && loading.value)
      page.value = "assistant";
  } catch (error) {
    accountMessage.value = accountErrorMessage(error);
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
    loginOpen.value = !selectedUser.value;
    page.value = selectedUser.value ? "home" : "settings";
  } catch (error) {
    accountMessage.value = accountErrorMessage(error);
  } finally {
    loading.value = false;
    await focusSettings();
  }
}
async function login(input: EnterpriseLogin) {
  await accountAction(async () => {
    if (props.environment?.account)
      selectedUser.value = await props.environment.account.password(input);
    else await loginEnterprise(input);
  });
}
function fixtureAuthenticated(context: UserContext) {
  selectedUser.value = context;
  loginOpen.value = false;
  page.value = "home";
}
function showLogin() {
  props.environment?.account?.cancel();
  loginOpen.value = true;
}
async function guest() {
  if (props.environment) {
    selectedUser.value = await props.environment.identity.read();
  } else await accountAction(enterGuest);
  loginOpen.value = false;
  page.value = "home";
}
async function focusSettings() {
  await nextTick();
  // The page header is moved after its host ref has been bound.
  await nextTick();
  const title =
    page.value === "assistant"
      ? ".assistant-header-host h1"
      : ".workspace-page-header > h1, .settings .agent-settings-heading h1";
  content.value
    ?.querySelector<HTMLElement>(title)
    ?.focus({ preventScroll: true });
}
async function navigate(id: string) {
  page.value = id;
}
watch(selectedUser, (next, previous) => {
  if (accountEnabled && previous && !next) {
    page.value = "settings";
    loginOpen.value = true;
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
    if (!document.hidden) void appearance.refresh();
    if (!document.hidden && page.value === "settings" && host.available)
      void host.refresh();
    // Registration retires the preceding account before publishing the activated
    // identity. A passive refresh must not remount and cancel its owning view.
    if (
      !props.environment &&
      !loading.value &&
      !registrationPending.value &&
      !!selectedUser.value
    ) {
      const epoch = registrationEpoch;
      void refreshAccount(
        () => epoch === registrationEpoch && !registrationPending.value,
      );
    }
  }, 2000);
});
onBeforeUnmount(() => {
  if (polling) clearInterval(polling);
  host.dispose();
  appearance.dispose();
});
</script>
<template>
  <UiProvider
    class="agent-experience"
    :platform="desktopPlatform"
    :reduced-motion="appearance.state.reducedMotion"
    :high-contrast="appearance.state.highContrast"
  >
    <div
      ref="content"
      class="app-root"
      :class="{
        'native-material': appearance.state.materialEnabled,
        'reduced-motion': appearance.state.reducedMotion,
        'high-contrast': appearance.state.highContrast,
      }"
    >
      <LoginScreen
        v-if="loginOpen"
        :loading="loading"
        :message="accountMessage || accountNotice"
        :fixture="environment?.account"
        @login="login"
        @guest="guest"
        @recover="(clear) => accountAction(() => recoverAccount(clear))"
        @authenticated="fixtureAuthenticated"
        @settings="
          loginOpen = false;
          page = 'settings';
        "
      >
        <template #login-qr><slot name="login-qr" /></template>
        <template v-if="selectedUser" #back
          ><button @click="loginOpen = false">返回工作区</button></template
        >
      </LoginScreen>
      <Workspace
        v-else
        :inert="loading ? true : undefined"
        :key="workspaceIdentity(selectedUser)"
        :ready="
          !registrationPending &&
          (environment ? !!selectedUser : !accountEnabled || !!selectedUser)
        "
        :busy="loading"
        :assistant-services="assistantServices"
        :self-service-port="selfServicePort"
        :page="page"
        :host="host"
        :service-port="environment?.service ?? servicePort"
        :device-port="environment?.device"
        :local-ports="environment?.local"
        :identity="selectedUser"
        :fixture="!!environment"
        @navigate="navigate"
      >
        <template #operation-cards="slotProps"
          ><slot
            name="operation-cards"
            :visible="slotProps.visible"
            :session-id="slotProps.sessionId"
            :drafts="slotProps.drafts"
        /></template>
        <template #account-summary>
          <strong>{{
            environment
              ? "本地工作区"
              : (selectedUser?.user.displayName ?? "账户与连接")
          }}</strong>
          <small>{{
            selectedUser?.identity.mode === "user_device"
              ? "企业工作区"
              : selectedUser?.identity.mode === "device"
                ? "不登录使用"
                : selectedUser
                  ? "测试用户 · 非企业认证"
                  : "选择使用身份"
          }}</small>
        </template>
        <template #development>
          <details
            v-if="environment"
            open
            class="fixture-controls"
            aria-label="开发 fixture"
          >
            <summary>场景与测试工具</summary>
            <div>
              <slot
                name="fixture-controls"
                :navigate="navigate"
                :login="showLogin"
              />
            </div>
          </details>
        </template>
        <template #user>
          <div v-if="environment">
            <p>开发 fixture 身份，不访问生产账户。</p>
            <button v-if="environment.account" @click="showLogin">
              预览企业登录
            </button>
          </div>
          <template v-else>
            <Account
              summary
              :loading="loading"
              :message="accountMessage || accountNotice"
              @guest="accountAction(enterGuest)"
              @logout="accountAction(logoutAccount)"
              @open-login="showLogin"
              @login="login"
            />
            <details class="account-local-workspaces">
              <summary>本地测试工作区</summary>
              <TestUsers
                :users="users"
                :current="
                  selectedUser?.identity.authorityId === 'desktop-test'
                    ? selectedUser
                    : undefined
                "
                :native="accountEnabled"
                :loading="loading"
                :message="message"
                @select="select"
              />
            </details>
          </template>
        </template>
      </Workspace>
    </div>
    <div
      v-if="loginOpen && environment?.account"
      class="login-fixture-controls"
      aria-label="Fixture 登录事件"
    >
      <span>Fixture 登录演示</span>
      <button @click="environment.account.scan()">模拟扫码</button
      ><button @click="environment.account.confirm()">手机确认</button
      ><button @click="environment.account.reject()">手机拒绝</button
      ><button @click="environment.account.expire()">二维码过期</button>
      <select
        aria-label="本机应用登录场景"
        @change="
          environment.account.scenario(
            ($event.target as HTMLSelectElement).value as 'normal',
          )
        "
      >
        <option value="normal">已登录</option>
        <option value="signed_out">全部未登录</option>
        <option value="missing">未检测到应用</option>
        <option value="mismatch">组织不匹配</option>
        <option value="expired">授权时失效</option>
        <option value="error">检测失败</option>
      </select>
    </div>
    <p v-if="loading" class="account-progress" role="status">
      正在读取或切换账户…
    </p>
  </UiProvider>
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
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-bg);
  color: var(--rss-color-text-muted);
  font: 12px var(--rss-font-sans);
}
.fixture-controls summary {
  cursor: pointer;
  padding: 6px 10px;
}
.fixture-controls > div {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  max-width: min(620px, 85vw);
  padding: 12px;
  border-top: 1px solid var(--rss-color-border);
}
.login-fixture-controls {
  position: fixed;
  bottom: 5px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  width: max-content;
  max-width: 95vw;
  border: 1px dashed var(--rss-color-accent);
  border-radius: 7px;
  padding: 7px 10px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text-muted);
  font-size: 10px;
  z-index: 20;
}
.login-fixture-controls button,
.login-fixture-controls select {
  font: inherit;
  color: var(--rss-color-text);
  background: var(--rss-color-bg);
  border: 1px solid var(--rss-color-border);
  border-radius: 5px;
  padding: 4px 7px;
  cursor: pointer;
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
