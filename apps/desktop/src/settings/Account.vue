<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  useId,
  watch,
} from "vue";
import { UiDialog, ChevronDown, UserRound } from "@rss-mdm-agent/ui";
import {
  accountBinding,
  currentUser,
  nativeTestMode,
  loadAccountSettings,
  saveOrganization,
  accountErrorMessage,
} from "../test-users";
import type {
  AccountOrganization,
  UserContext,
} from "@rss-mdm-agent/ai-contract/wire";
import type { EnterpriseLogin, FixtureLoginPort } from "../account/types";
const props = withDefaults(
  defineProps<{
    loading: boolean;
    message?: string;
    registration?: boolean;
    summary?: boolean;
    fixture?: FixtureLoginPort;
  }>(),
  { message: "" },
);
const emit = defineEmits<{
  login: [input: EnterpriseLogin];
  guest: [];
  logout: [];
  "open-login": [];
  recover: [clear: boolean];
  authenticated: [context: UserContext];
}>();
const uid = useId();
const tabButtons = ref<HTMLButtonElement[]>([]);
const organizations = ref<AccountOrganization[]>([]),
  selected = ref(""),
  login = ref(""),
  password = ref(""),
  reveal = ref(false),
  consent = ref(false);
const method = ref<"password" | "qr" | "client">("password");
const choosing = ref(false),
  adding = ref(false),
  help = ref(false),
  label = ref(""),
  origin = ref(""),
  tenant = ref(""),
  localMessage = ref(""),
  saving = ref(false);
const available = computed(() => !!props.fixture || nativeTestMode);
const alternative = computed(() => !!props.fixture && !props.registration);
const activeOrganization = computed(() =>
  organizations.value.find((o) => o.id === selected.value),
);
const unresolved = computed(
  () =>
    !props.fixture &&
    ["binding_unknown", "cleanup_pending", "conflict", "invalidated"].includes(
      accountBinding.value,
    ),
);
const feedback = computed(() => localMessage.value || props.message);
async function load() {
  if (props.fixture) {
    organizations.value = props.fixture.organizations;
    selected.value = organizations.value[0]?.id ?? "";
    return;
  }
  if (!nativeTestMode) return;
  try {
    const value = await loadAccountSettings();
    organizations.value = value.organizations;
    selected.value = value.selected ?? value.organizations[0]?.id ?? "";
  } catch {
    localMessage.value = "组织配置不可用，请重试。";
  }
}
onMounted(load);
watch(selected, () => {
  password.value = "";
  localMessage.value = "";
  props.fixture?.organization(selected.value);
});
watch(
  () => props.fixture?.state.current,
  (value) => {
    if (value && method.value !== "password") emit("authenticated", value);
  },
);
watch(
  () => props.loading,
  (next, previous) => {
    if (next && !previous) password.value = "";
  },
);
onBeforeUnmount(() => {
  password.value = "";
  props.fixture?.cancel();
});
function submit() {
  if (props.loading || !available.value) return;
  if (!selected.value || !login.value.trim() || !password.value.trim()) {
    localMessage.value = "请选择组织，并填写企业账号与密码。";
    return;
  }
  if (props.registration && !consent.value) {
    localMessage.value = "请先同意将此设备注册到组织。";
    return;
  }
  const input = {
    organizationId: selected.value,
    login: login.value.trim(),
    password: password.value,
  };
  password.value = "";
  reveal.value = false;
  localMessage.value = "";
  emit("login", input);
}
async function changeMethod(next: typeof method.value) {
  if (props.loading || (!alternative.value && next !== "password")) return;
  password.value = "";
  reveal.value = false;
  method.value = next;
  localMessage.value = "";
  props.fixture?.cancel();
  if (next === "qr") props.fixture?.refreshQr();
  if (next === "client") await props.fixture?.discover();
}
async function clientLogin(id: string) {
  try {
    await props.fixture?.loginClient(id);
  } catch {
    localMessage.value = "应用登录确认未完成，请重新检测。";
  }
}
async function save() {
  if (saving.value) return;
  saving.value = true;
  try {
    const saved = await saveOrganization({
      id: "",
      label: label.value,
      origin: origin.value,
      tenantId: tenant.value,
    });
    organizations.value = [
      ...organizations.value.filter((o) => o.id !== saved.id),
      saved,
    ];
    selected.value = saved.id;
    choosing.value = false;
    adding.value = false;
    localMessage.value = "";
  } catch (error) {
    localMessage.value = accountErrorMessage(error);
  } finally {
    saving.value = false;
  }
}
function selectOrganization(id: string) {
  selected.value = id;
  choosing.value = false;
  method.value = "password";
  props.fixture?.cancel();
}
async function tabKeys(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const methods = ["password", "qr", "client"] as const;
  const index = methods.indexOf(method.value);
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? 2
        : (index + (event.key === "ArrowRight" ? 1 : 2)) % 3;
  const nextMethod = methods.at(next)!;
  await changeMethod(nextMethod);
  await nextTick();
  tabButtons.value
    .find((button) => button.id === `${uid}-tab-${nextMethod}`)
    ?.focus({ preventScroll: true });
}
</script>
<template>
  <section v-if="summary" class="account-summary" aria-label="企业账户">
    <div>
      <UserRound :size="20" /><span>{{
        currentUser?.identity.mode === "user_device"
          ? "企业身份已登录"
          : "使用本机工作区"
      }}</span>
    </div>
    <p v-if="message" role="status">{{ message }}</p>
    <button @click="emit('open-login')">
      {{
        currentUser?.identity.mode === "user_device"
          ? "切换企业账号"
          : "登录企业账号"
      }}
    </button>
    <button
      v-if="currentUser?.identity.mode === 'user_device'"
      :disabled="loading"
      @click="emit('logout')"
    >
      退出登录
    </button>
  </section>
  <section v-else class="account" aria-label="企业账户">
    <header class="auth-heading">
      <span class="eyebrow">YOUR WORKSPACE, YOUR WAY</span>
      <h2>{{ registration ? "连接组织，开始工作" : "欢迎回到工作空间" }}</h2>
      <p>
        {{
          registration
            ? "登录企业账号，并将这台电脑注册到组织。"
            : "登录你的组织，让常用工具与工作随时就绪。"
        }}
      </p>
      <button
        class="organization-card"
        :disabled="loading"
        @click="choosing = true"
      >
        <span class="organization-icon">{{
          activeOrganization?.label.slice(0, 1) ?? "企"
        }}</span
        ><span
          ><strong>{{ activeOrganization?.label ?? "选择你的工作组织" }}</strong
          ><small>{{
            activeOrganization?.origin ?? "使用管理员提供的组织连接"
          }}</small></span
        ><ChevronDown :size="15" />
      </button>
    </header>
    <div
      v-if="alternative"
      class="login-tabs"
      role="tablist"
      aria-label="登录方式"
      @keydown="tabKeys"
    >
      <button
        v-for="item in [
          { id: 'password', label: '账号密码' },
          { id: 'qr', label: '扫码登录' },
          { id: 'client', label: '本机快捷登录' },
        ] as const"
        ref="tabButtons"
        :id="`${uid}-tab-${item.id}`"
        :key="item.id"
        role="tab"
        :aria-selected="method === item.id"
        :aria-controls="`${uid}-panel-${item.id}`"
        :tabindex="method === item.id ? 0 : -1"
        :disabled="loading"
        @click="changeMethod(item.id)"
      >
        {{ item.label }}
      </button>
    </div>
    <div v-else class="login-label">企业账号登录</div>
    <div class="login-stage">
      <div
        :id="`${uid}-panel-${method}`"
        class="login-pane"
        :role="alternative ? 'tabpanel' : undefined"
        :aria-labelledby="alternative ? `${uid}-tab-${method}` : undefined"
      >
        <form v-if="method === 'password'" @submit.prevent="submit">
          <label :for="`${uid}-account`">企业账号</label
          ><input
            :id="`${uid}-account`"
            v-model="login"
            aria-label="企业账号"
            name="username"
            autocomplete="username"
            placeholder="工作邮箱或企业账号"
            maxlength="256"
            :disabled="loading || !available || unresolved"
          />
          <div class="label-line">
            <label :for="`${uid}-password`">密码</label
            ><button type="button" class="text-link" @click="help = true">
              忘记密码？
            </button>
          </div>
          <div class="password-input">
            <input
              :id="`${uid}-password`"
              v-model="password"
              aria-label="企业账号密码"
              name="password"
              :type="reveal ? 'text' : 'password'"
              autocomplete="current-password"
              placeholder="输入账号密码"
              maxlength="16384"
              :disabled="loading || !available || unresolved"
            /><button
              type="button"
              :aria-label="reveal ? '隐藏密码' : '显示密码'"
              :aria-pressed="reveal"
              @click="reveal = !reveal"
            >
              {{ reveal ? "隐藏" : "显示" }}
            </button>
          </div>
          <label v-if="registration" class="consent"
            ><input
              v-model="consent"
              type="checkbox"
              :disabled="loading"
            />我同意将此设备注册到组织，允许组织管理工作应用与设备配置。</label
          >
          <button
            class="login-primary"
            :disabled="loading || !available || unresolved"
          >
            {{
              loading
                ? "正在安全登录…"
                : registration
                  ? "登录并注册此设备"
                  : "登录工作空间"
            }}
            <span v-if="!loading">→</span>
          </button>
          <div class="login-feedback" :role="feedback ? 'alert' : 'status'">
            {{ feedback || "登录后，此账号将成为受管设备的当前使用者。" }}
          </div>
          <div v-if="unresolved" class="recovery-actions">
            <button
              type="button"
              :disabled="loading"
              @click="emit('recover', false)"
            >
              重试原操作</button
            ><button
              type="button"
              class="text-link"
              :disabled="loading"
              @click="emit('recover', true)"
            >
              清理后重新登录
            </button>
          </div>
        </form>
        <div v-else-if="method === 'qr' && fixture" class="qr-pane">
          <p>使用已登录企业账号的手机扫码确认</p>
          <div class="qr-box">
            <slot name="qr" />
            <div v-if="fixture.state.qr !== 'waiting'" class="qr-overlay">
              <strong>{{
                fixture.state.qr === "scanned"
                  ? "已扫码，请在手机上确认"
                  : fixture.state.qr === "expired"
                    ? "二维码已过期"
                    : "本次登录已取消"
              }}</strong
              ><button
                v-if="fixture.state.qr !== 'scanned'"
                class="text-link"
                @click="fixture.refreshQr()"
              >
                刷新二维码
              </button>
            </div>
          </div>
          <p role="status">
            {{
              fixture.state.qr === "waiting" ? "等待扫码" : "仅用于登录交互演示"
            }}
          </p>
          <small>在手机上确认后，这里将自动继续。</small>
        </div>
        <div v-else-if="method === 'client' && fixture" class="client-pane">
          <div class="client-heading">
            <span>使用本机应用中已登录的企业账号</span
            ><button
              class="text-link"
              :disabled="fixture.state.discovering"
              @click="fixture.discover()"
            >
              重新检测
            </button>
          </div>
          <p
            v-if="fixture.state.discovering"
            class="client-empty"
            role="status"
          >
            正在读取本机应用登录状态…
          </p>
          <div v-else-if="!fixture.state.clients.length" class="client-empty">
            未检测到支持快捷登录的应用
          </div>
          <div v-else class="client-list">
            <article
              v-for="client in fixture.state.clients"
              :key="client.id"
              class="client-card"
            >
              <span
                class="client-icon"
                :class="{ zt: client.kind === '零信任' }"
                >{{ client.kind === "IM" ? "企" : "盾" }}</span
              >
              <div>
                <strong
                  >{{ client.name }}<small>{{ client.kind }}</small></strong
                >
                <p>
                  {{
                    client.status === "signed_in"
                      ? `已登录 · ${client.account}`
                      : client.status === "mismatch"
                        ? "与当前组织不匹配"
                        : client.status === "expired"
                          ? "登录已失效"
                          : "尚未登录"
                  }}
                </p>
              </div>
              <button
                v-if="client.status === 'signed_in'"
                class="client-login"
                :disabled="fixture.state.discovering"
                @click="clientLogin(client.id)"
              >
                一键登录</button
              ><span v-else>不可用</span>
            </article>
          </div>
          <p class="login-feedback" role="status">
            {{
              fixture.state.error ||
              "由组织授权的应用确认身份，无需再次输入密码。"
            }}
          </p>
        </div>
      </div>
    </div>
    <footer v-if="!registration" class="account-footer">
      <button :disabled="loading || !available" @click="emit('guest')">
        暂不登录，使用本机工作区</button
      ><small>企业资源将在登录后提供。</small>
    </footer>
    <UiDialog
      v-model:open="choosing"
      title="选择工作组织"
      description="使用对应组织的企业账号登录。"
      ><button
        v-for="item in organizations"
        :key="item.id"
        class="organization-row"
        @click="selectOrganization(item.id)"
      >
        <strong>{{ item.label }}</strong
        ><small>{{ item.origin }}</small
        ><span v-if="selected === item.id">✓</span></button
      ><button
        v-if="!fixture"
        class="text-link add-organization"
        @click="adding = !adding"
      >
        {{ adding ? "收起连接设置" : "+ 连接其他组织" }}
      </button>
      <form v-if="adding" class="organization-form" @submit.prevent="save">
        <label
          >组织名称<input
            v-model="label"
            aria-label="组织名称"
            required
            maxlength="64" /></label
        ><label
          >组织服务地址<input
            v-model="origin"
            aria-label="组织服务地址"
            type="url"
            placeholder="https://workspace.company"
            required
        /></label>
        <details>
          <summary>高级连接设置</summary>
          <label
            >租户 ID<input
              v-model="tenant"
              aria-label="租户 UUID"
              placeholder="管理员提供的租户 UUID"
              required
          /></label>
        </details>
        <p v-if="localMessage" role="alert">{{ localMessage }}</p>
        <button class="login-primary" :disabled="saving">保存组织连接</button>
      </form></UiDialog
    >
    <UiDialog
      v-model:open="help"
      title="找回企业账号密码"
      description="企业账号由组织管理。"
      ><p>
        请联系 {{ activeOrganization?.label ?? "组织" }} 的 IT
        管理员重置密码，然后返回这里登录。
      </p></UiDialog
    >
  </section>
</template>
<style scoped>
.account {
  width: 100%;
  max-width: 348px;
  color: var(--rss-color-text);
  font-size: 12px;
}
.auth-heading .eyebrow {
  display: block;
  color: var(--rss-color-text-muted);
  font-size: 9px;
  letter-spacing: 1.6px;
  margin-bottom: 8px;
}
.auth-heading h2 {
  margin: 0;
  font-size: 27px;
  line-height: 1.45;
  letter-spacing: -0.7px;
  font-weight: 600;
}
.auth-heading > p {
  font-size: 11px;
  color: var(--rss-color-text-muted);
  line-height: 1.9;
  margin: 7px 0 0;
}
.organization-card {
  display: flex;
  align-items: center;
  gap: 11px;
  height: 58px;
  width: 100%;
  margin: 19px 0 17px;
  padding: 10px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 9px;
  background: var(--rss-color-bg);
  text-align: left;
}
.organization-icon {
  width: 33px;
  height: 33px;
  background: #e9eee7;
  color: #5e7564;
  display: grid;
  place-items: center;
  border-radius: 9px;
  font-size: 14px;
}
.organization-card > span:nth-child(2) {
  flex: 1;
  min-width: 0;
}
.organization-card strong {
  display: block;
  font-size: 12px;
  font-weight: 550;
}
.organization-card small {
  display: block;
  font-size: 10px;
  color: var(--rss-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
  overflow: hidden;
}
.login-tabs {
  display: flex;
  gap: 24px;
  height: 37px;
  border-bottom: 1px solid var(--rss-color-border);
  margin-bottom: 17px;
}
.account .login-tabs button {
  min-height: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  height: 37px;
  position: relative;
  padding: 0 0 11px;
  color: var(--rss-color-text-muted);
  white-space: nowrap;
  font-size: 11px;
}
.login-tabs button[aria-selected="true"] {
  color: var(--rss-color-accent);
  font-weight: 600;
}
.login-tabs button[aria-selected="true"]:after {
  content: "";
  position: absolute;
  bottom: -1px;
  height: 2px;
  left: 0;
  right: 0;
  background: var(--rss-color-accent);
}
.login-label {
  height: 37px;
  border-bottom: 1px solid var(--rss-color-border);
  margin-bottom: 17px;
  color: var(--rss-color-text-muted);
  font-size: 11px;
  display: flex;
  align-items: flex-start;
}
.login-stage {
  height: 324px;
  min-height: 324px;
  max-height: 324px;
}
.login-pane {
  height: 100%;
}
.account form > label,
.label-line label {
  display: block;
  margin-bottom: 7px;
  font-size: 11px;
  font-weight: 550;
}
.account input:not([type="checkbox"]),
.organization-form input {
  width: 100%;
  min-height: 44px;
  padding: 11px 13px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
  font: inherit;
  font-size: 12px;
}
.account input:focus {
  outline: 2px solid
    color-mix(in srgb, var(--rss-color-accent) 25%, transparent);
  border-color: var(--rss-color-accent);
}
.label-line {
  display: flex;
  justify-content: space-between;
  margin-top: 15px;
}
.account .text-link {
  min-height: 0;
  border: 0;
  background: transparent;
  color: var(--rss-color-accent);
  font-size: 10px;
  padding: 0;
}
.password-input {
  position: relative;
}
.password-input input {
  padding-right: 48px !important;
}
.account .password-input button {
  min-height: 0;
  border: 0;
  background: transparent;
  position: absolute;
  right: 10px;
  top: 12px;
  font-size: 10px;
  color: var(--rss-color-text-muted);
  padding: 0;
}
.login-primary {
  display: flex;
  justify-content: center;
  gap: 9px;
  align-items: center;
  width: 100%;
  min-height: 44px;
  background: #3973e8;
  color: white;
  border: 0;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 550;
  margin-top: 18px;
  box-shadow: 0 3px 6px #3973e818;
}
.login-primary:hover {
  background: #2e63d1;
}
.login-feedback {
  height: 46px;
  margin: 0;
  padding-top: 11px;
  font-size: 10px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
}
.login-feedback[role="alert"] {
  color: #b84e55;
}
.recovery-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 11px;
}
.recovery-actions > button:first-child {
  color: var(--rss-color-accent);
}
.account-footer {
  height: 66px;
  border-top: 1px solid var(--rss-color-border);
  padding-top: 6px;
  text-align: center;
}
.account .account-footer > button {
  border: 0;
  background: transparent;
  height: 36px;
  width: 100%;
  font-size: 11px;
  color: var(--rss-color-text-muted);
}
.account-footer > small {
  display: block;
  font-size: 9px;
  color: var(--rss-color-text-muted);
}
button {
  font: inherit;
  cursor: pointer;
  border: 0;
  background: transparent;
  color: inherit;
}
button:disabled,
input:disabled {
  opacity: 0.6;
  cursor: default;
}
button:focus-visible,
summary:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: 3px;
}
.client-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 28px;
  font-size: 10px;
  margin-bottom: 8px;
  color: var(--rss-color-text-muted);
}
.client-list {
  display: grid;
  gap: 8px;
}
.client-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 11px;
  height: 66px;
  border: 1px solid var(--rss-color-border);
  border-radius: 9px;
}
.client-icon {
  width: 31px;
  height: 31px;
  border-radius: 8px;
  background: #e9f2ff;
  color: #3d75c0;
  display: grid;
  place-items: center;
  font-size: 12px;
  flex: none;
}
.client-icon.zt {
  background: #f0ebf9;
  color: #7a65aa;
}
.client-card > div {
  flex: 1;
  min-width: 0;
}
.client-card strong {
  display: block;
  font-size: 11px;
  font-weight: 550;
  white-space: nowrap;
}
.client-card strong small {
  font-size: 8px;
  font-weight: 400;
  color: var(--rss-color-text-muted);
  margin-left: 6px;
}
.client-card p {
  font-size: 9px;
  color: var(--rss-color-text-muted);
  margin: 4px 0 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.client-card > span:last-child {
  font-size: 9px;
  color: var(--rss-color-text-muted);
}
.client-login {
  font-size: 10px;
  padding: 6px 8px;
  border-radius: 5px;
  color: var(--rss-color-accent);
  background: var(--rss-color-accent-bg);
  white-space: nowrap;
}
.client-empty {
  height: 214px;
  display: grid;
  place-items: center;
  border: 1px solid var(--rss-color-border);
  border-radius: 9px;
  background: var(--rss-color-bg);
  font-size: 11px;
  color: var(--rss-color-text-muted);
  margin: 0;
}
.qr-pane {
  text-align: center;
}
.qr-pane > p {
  font-size: 11px;
  color: var(--rss-color-text-muted);
  margin: 0 0 14px;
}
.qr-pane > p[role="status"] {
  margin: 12px 0 4px;
}
.qr-pane > small {
  font-size: 10px;
  color: var(--rss-color-text-muted);
}
.qr-box {
  position: relative;
  width: 190px;
  height: 190px;
  padding: 8px;
  margin: 0 auto;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: white;
}
.qr-box :deep(img) {
  width: 172px;
  height: 172px;
}
.qr-overlay {
  position: absolute;
  inset: 0;
  background: color-mix(in srgb, var(--rss-color-surface) 96%, transparent);
  backdrop-filter: blur(3px);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  border-radius: 11px;
  padding: 16px;
}
.qr-overlay strong {
  font-size: 12px;
  font-weight: 550;
}
.consent {
  display: flex !important;
  align-items: flex-start;
  gap: 8px;
  font-size: 10px !important;
  line-height: 1.7;
  margin: 13px 0 0 !important;
}
.consent input {
  margin-top: 2px;
  accent-color: #3973e8;
}
.organization-row {
  display: block;
  position: relative;
  text-align: left;
  width: 100%;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  padding: 14px;
  margin: 12px 0;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
}
.organization-row strong {
  display: block;
  font-size: 12px;
  font-weight: 550;
}
.organization-row small {
  font-size: 10px;
  color: var(--rss-color-text-muted);
}
.organization-row > span {
  position: absolute;
  right: 14px;
  top: 20px;
  color: var(--rss-color-accent);
}
.add-organization {
  margin-top: 10px;
}
.organization-form {
  margin-top: 15px;
  font-size: 12px;
}
.organization-form label {
  display: block;
  margin-top: 12px;
}
.organization-form input {
  display: block;
  margin-top: 7px;
}
.organization-form details {
  margin-top: 15px;
}
.organization-form summary {
  cursor: pointer;
}
.account-summary {
  font-size: 12px;
  line-height: 1.9;
}
.account-summary > div {
  display: flex;
  align-items: center;
  gap: 9px;
}
.account-summary button {
  padding: 8px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  margin: 12px 8px 0 0;
}
.account-summary p {
  color: var(--rss-color-text-muted);
}
@media (max-width: 680px) {
  .auth-heading h2 {
    font-size: 25px;
  }
  .login-tabs {
    gap: 21px;
  }
  .login-tabs button {
    font-size: 10px;
  }
  .client-card {
    padding: 10px 9px;
    gap: 8px;
  }
  .client-card strong {
    font-size: 10px;
  }
  .client-card p {
    font-size: 8px;
  }
  .client-login {
    font-size: 9px;
    padding: 6px;
  }
  .organization-card {
    margin-top: 18px;
  }
}
</style>
