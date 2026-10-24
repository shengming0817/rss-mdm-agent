<script setup lang="ts">
import { reactive, computed, nextTick, ref, watch } from "vue";
import { UserRound } from "@rss-mdm-agent/ui";
import type { AssistantController } from "../assistant/controller";
import type { DeviceOverview, SelfServicePort } from "../self-service/types";
import type { PageNavigation } from "../workspace/page-navigation";
import DiagnosticsPanel from "./DiagnosticsPanel.vue";
import LocalService from "./LocalService.vue";
import type { ServicePort } from "./native";
import ConnectionSettings from "./ConnectionSettings.vue";
import SessionDiagnostics from "./SessionDiagnostics.vue";
import SessionConnection from "../assistant/SessionConnection.vue";
import { diagnosticMessage, type HostSettings } from "./controller";
const props = withDefaults(
  defineProps<{
    host: HostSettings;
    servicePort?: ServicePort;
    selfServicePort?: SelfServicePort;
    device?: DeviceOverview | null;
    fixture?: boolean;
    active?: boolean;
    assistant?: AssistantController;
  }>(),
  { active: true },
);
const emit = defineEmits<{
  navigation: [state: PageNavigation];
  assistant: [];
  tasks: [];
}>();
const section = ref("account");
const root = ref<HTMLElement>();
const sections = [
  { id: "account", title: "账号与组织" },
  { id: "registration", title: "设备注册与 MDM", kind: "display" },
  { id: "connection", title: "AI 与连接" },
  { id: "diagnostics", title: "运行与诊断", kind: "tools" },
  { id: "about", title: "关于与隐私" },
  ...(props.fixture ? [{ id: "development", title: "开发预览" }] : []),
];
const path = computed(() => [
  {
    id: section.value,
    title: sections.find((e) => e.id === section.value)!.title,
  },
]);
async function locate() {
  await nextTick();
  await nextTick();
  const container = root.value;
  const target = container?.querySelector<HTMLElement>(
    `[data-settings-section="${section.value}"]`,
  );
  if (!container || !target || !props.active) return;
  container.scrollTop =
    section.value === "account"
      ? 0
      : container.scrollTop +
        target.getBoundingClientRect().top -
        container.getBoundingClientRect().top -
        24;
  target.querySelector<HTMLElement>("h2")?.focus({ preventScroll: true });
}
function openSection(id: string) {
  if (!sections.some((e) => e.id === id)) return;
  section.value = id;
  void locate();
}
function reset() {
  openSection("account");
}
function trackSection() {
  const container = root.value;
  if (!container?.clientHeight || !props.active) return;
  if (
    container.scrollTop > 0 &&
    Math.ceil(container.scrollTop + container.clientHeight) >=
      container.scrollHeight - 1
  ) {
    section.value = sections.at(-1)!.id;
    return;
  }
  const threshold = container.getBoundingClientRect().top + 96;
  const current = [...sections].reverse().find((item) => {
    const target = container.querySelector(
      `[data-settings-section="${item.id}"]`,
    );
    return target && target.getBoundingClientRect().top <= threshold;
  });
  section.value = current?.id ?? "account";
}
watch(
  () => props.active,
  (active) => {
    if (active) void locate();
  },
  { immediate: true },
);
const confirming = ref(false),
  dialog = ref<HTMLElement>(),
  trigger = ref<HTMLElement>();
const phases = new Map([
  ["ready", "已就绪"],
  ["starting", "正在启动"],
  ["stopping", "正在关闭"],
  ["stopped", "已停止"],
  ["failed", "不可用"],
]);
const readyConnection = computed(() =>
  props.assistant?.state.connections.some((c) => c.status === "ready"),
);
function newConversation() {
  props.assistant?.create();
  emit("assistant");
}
async function confirm() {
  confirming.value = true;
  await nextTick();
  dialog.value?.querySelector<HTMLElement>("button")?.focus();
}
async function close() {
  confirming.value = false;
  await nextTick();
  trigger.value?.focus({ preventScroll: true });
}
async function restart() {
  await close();
  await props.host.restart();
  await nextTick();
  trigger.value?.focus({ preventScroll: true });
}
function keys(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    void close();
  }
  if (event.key !== "Tab") return;
  const buttons = [
    ...(dialog.value?.querySelectorAll<HTMLElement>("button") ?? []),
  ];
  const first = buttons[0],
    last = buttons.at(-1);
  if (event.shiftKey && event.target === first) {
    event.preventDefault();
    last?.focus();
  }
  if (!event.shiftKey && event.target === last) {
    event.preventDefault();
    first?.focus();
  }
}
emit("navigation", reactive({ openSection, reset, section, sections, path }));
</script>
<template>
  <section
    ref="root"
    class="settings settings-page design-page"
    aria-label="RSS Agent 设置"
    @scroll.passive="trackSection"
  >
    <div class="settings-document" :inert="confirming ? true : undefined">
      <header class="agent-settings-heading">
        <h1 tabindex="-1">设置</h1>
        <p>管理账户、设备注册与电脑助手。</p>
      </header>
      <section
        class="settings-section"
        data-settings-section="account"
        aria-label="账号与组织"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">账号与组织</h2>
          <p>管理登录身份和组织连接。</p>
        </header>
        <div class="settings-surface account-surface">
          <div class="account-profile">
            <span class="profile-avatar"
              ><UserRound :size="24" aria-hidden="true"
            /></span>
            <div class="profile-identity">
              <slot name="account-summary"
                ><strong>账户与连接</strong><small>选择使用身份</small></slot
              >
            </div>
          </div>
          <div class="account-options"><slot name="user" /></div>
        </div>
      </section>
      <section
        class="settings-section"
        data-settings-section="registration"
        aria-label="设备注册与 MDM"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">设备注册与 MDM</h2>
          <p>查看设备身份，注册组织的 Agent 与 MDM 管理。</p>
        </header>
        <div class="settings-surface device-overview">
          <dl class="settings-facts">
            <div>
              <dt>设备名称</dt>
              <dd>{{ device?.attributes.computerName ?? "尚未读取" }}</dd>
            </div>
            <div>
              <dt>当前系统用户</dt>
              <dd>{{ device?.attributes.loginUser ?? "尚未读取" }}</dd>
            </div>
            <div>
              <dt>IP 地址</dt>
              <dd>
                {{ device?.attributes.ipAddresses?.join("、") || "尚未读取" }}
              </dd>
            </div>
          </dl>
        </div>
        <LocalService
          :port="servicePort"
          :execution-port="selfServicePort"
          :fixture="fixture"
          :active="active"
        />
      </section>
      <section
        class="settings-section"
        data-settings-section="connection"
        aria-label="AI 与连接"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">AI 与连接</h2>
          <p>管理模型服务、默认连接与会话上下文。</p>
        </header>
        <div class="settings-surface ai-settings">
          <template v-if="assistant">
            <p v-if="fixture" class="settings-note">
              开发预览连接仅用于隔离样例。
            </p>
            <ConnectionSettings :controller="assistant" :active="active" />
            <details class="settings-disclosure" open>
              <summary>当前会话连接与上下文</summary>
              <SessionConnection :controller="assistant" />
            </details>
            <div class="settings-actions">
              <button
                :disabled="
                  !readyConnection ||
                  assistant.state.connection !== 'connected' ||
                  assistant.state.opening
                "
                @click="newConversation"
              >
                开始对话</button
              ><button @click="emit('assistant')">稍后配置，前往 AI</button>
            </div>
            <p v-if="assistant.state.createError" role="alert">
              新建会话未完成，请重新连接后重试。
            </p>
            <details v-if="active" class="settings-disclosure">
              <summary>会话详情与诊断</summary>
              <SessionDiagnostics :controller="assistant" />
            </details>
          </template>
          <p v-else class="settings-note">
            登录或选择使用身份后，可配置 AI 连接。
          </p>
        </div>
      </section>
      <section
        class="settings-section"
        data-settings-section="diagnostics"
        aria-label="运行与诊断"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">运行与诊断</h2>
          <p>检查 AI 服务状态，恢复连接或导出脱敏诊断。</p>
        </header>
        <div class="settings-surface">
          <dl class="settings-facts">
            <div>
              <dt>AI 服务</dt>
              <dd>
                {{
                  host.state.status
                    ? phases.get(host.state.status.phase)
                    : host.available
                      ? "正在读取状态"
                      : "桌面应用中可用"
                }}
              </dd>
            </div>
            <div v-if="host.state.status">
              <dt>版本</dt>
              <dd>{{ host.state.status.version }}</dd>
            </div>
          </dl>
          <p v-if="diagnosticMessage(host.state.status)" role="alert">
            {{ diagnosticMessage(host.state.status) }}
          </p>
          <div class="settings-actions">
            <button v-if="assistant" @click="emit('tasks')">
              执行服务记录
            </button>
            <button
              :disabled="
                !host.available || host.state.busy || host.state.loading
              "
              @click="host.refresh"
            >
              刷新状态</button
            ><button
              ref="trigger"
              :disabled="
                !host.available || !host.state.status || host.state.busy
              "
              @click="confirm"
            >
              重启 AI Host</button
            ><button
              v-if="assistant"
              :disabled="
                assistant.state.connection === 'connecting' || host.state.busy
              "
              @click="assistant.connect"
            >
              重新连接
            </button>
          </div>
          <DiagnosticsPanel
            :active="active"
            :port="fixture ? null : undefined"
          />
          <p v-if="host.state.busy" role="status">正在处理，请稍候…</p>
          <p v-if="host.state.readError" role="alert">
            {{ host.state.readError }}
          </p>
          <p v-if="host.state.message" role="status">
            {{ host.state.message }}
          </p>
        </div>
      </section>
      <section
        class="settings-section"
        data-settings-section="about"
        aria-label="关于与隐私"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">关于与隐私</h2>
          <p>了解应用版本与数据的使用方式。</p>
        </header>
        <div class="settings-surface">
          <div class="about-product">
            <span class="agent-mark">r.</span>
            <div>
              <strong>RSS Agent</strong>
              <p>桌面电脑助手 · MIT 许可</p>
            </div>
          </div>
          <dl class="privacy-rows">
            <div>
              <dt>账户与历史</dt>
              <dd>保存在本机，切换身份不会合并数据。</dd>
            </div>
            <div>
              <dt>AI 请求</dt>
              <dd>发送到所选服务，跨连接携带历史需由你确认。</dd>
            </div>
            <div>
              <dt>凭据</dt>
              <dd>API 密钥由原生层加密保存；官方工具使用自己的登录配置。</dd>
            </div>
            <div>
              <dt>设备任务</dt>
              <dd>
                由本机系统服务执行，独立核实结果；切换身份不会改变任务归属。
              </dd>
            </div>
          </dl>
        </div>
      </section>
      <section
        v-if="fixture"
        class="settings-section development-settings"
        data-settings-section="development"
        aria-label="开发预览"
      >
        <header class="settings-section-heading">
          <h2 tabindex="-1">开发预览</h2>
          <p>隔离样例与验证工具，不访问生产账户或执行真实设备操作。</p>
        </header>
        <div class="settings-surface">
          <slot name="development" :active="active" />
        </div>
      </section>
    </div>
    <section
      v-if="confirming"
      ref="dialog"
      class="settings-confirm"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="restart-title"
      @keydown="keys"
    >
      <h2 id="restart-title">重启 AI Host</h2>
      <p>
        模型请求可能中断，未知结果不会自动重发。设备任务继续，不会被标记为取消或完成。
      </p>
      <div class="settings-actions">
        <button @click="close">返回设置</button
        ><button @click="restart">确认重启</button>
      </div>
    </section>
  </section>
</template>
<style scoped>
/* ref: vscode 1.105.0 preferences/browser/media/settingsEditor2.css — one document with section navigation. */
.settings.settings-page {
  height: 100%;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  padding: 32px 36px 56px;
}
.settings-document {
  max-width: 940px;
  margin: 0 auto;
}
.agent-settings-heading {
  margin-bottom: 36px;
}
.agent-settings-heading h1 {
  margin: 0 0 9px;
  font-size: 27px;
  font-weight: 600;
  letter-spacing: -0.6px;
}
.agent-settings-heading p,
.settings-section-heading p {
  color: var(--rss-color-text-muted);
  margin: 0;
  font-size: 12px;
  line-height: 1.7;
}
.settings-section {
  margin-top: 36px;
  scroll-margin-top: 24px;
  min-width: 0;
}
.settings-section-heading {
  margin-bottom: 14px;
}
.settings-section-heading h2 {
  margin: 0 0 6px;
  font-size: 17px;
  font-weight: 600;
  letter-spacing: -0.2px;
}
.settings-surface {
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  padding: 22px 24px;
  min-width: 0;
  overflow-wrap: anywhere;
}
.settings-surface + .settings-surface {
  margin-top: 12px;
}
.settings-facts {
  margin: 0;
  display: grid;
  gap: 0;
}
.settings-facts > div {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr);
  gap: 16px;
  padding: 11px 0;
  font-size: 12px;
  line-height: 1.7;
}
.settings-facts > div:first-child {
  padding-top: 0;
}
.settings-facts > div:last-child {
  padding-bottom: 0;
}
dt {
  color: var(--rss-color-text-muted);
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.settings-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 18px;
}
.settings :is(button, input, select) {
  font: inherit;
}
.settings .settings-actions > button,
.settings-confirm button {
  min-height: 34px;
  padding: 7px 12px;
  margin: 0;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  background: var(--rss-color-surface);
  font-size: 12px;
  color: var(--rss-color-text);
}
.settings .settings-actions > button:hover:enabled {
  background: var(--rss-color-neutral-bg);
}
.settings-note,
.settings-surface > p {
  font-size: 11px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
}
.settings-disclosure {
  border-top: 1px solid var(--rss-color-border);
  margin-top: 22px;
  padding-top: 18px;
}
.settings-disclosure > summary {
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
}
.settings-disclosure > :not(summary) {
  margin-top: 18px;
}
.account-profile {
  display: flex;
  align-items: center;
  gap: 14px;
  padding-bottom: 22px;
  border-bottom: 1px solid var(--rss-color-border);
  margin-bottom: 22px;
}
.profile-avatar {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  flex: none;
  border-radius: 50%;
  color: var(--rss-color-text-muted);
  background: var(--rss-color-neutral-bg);
}
.profile-identity {
  min-width: 0;
}
.profile-identity :deep(strong) {
  display: block;
  font-size: 15px;
  font-weight: 600;
}
.profile-identity :deep(small) {
  display: block;
  margin-top: 5px;
  color: var(--rss-color-text-muted);
  font-size: 11px;
}
.account-options {
  display: grid;
  gap: 20px;
}
.account-options :deep(> .account) {
  border: 0;
  padding: 0;
}
.account-options :deep(> p) {
  font-size: 12px;
  margin: 0;
  color: var(--rss-color-text-muted);
}
.account-options :deep(.account-local-workspaces) {
  border-top: 1px solid var(--rss-color-border);
  padding-top: 18px;
}
.account-options :deep(.account-local-workspaces > summary) {
  cursor: pointer;
  font-size: 12px;
  color: var(--rss-color-text-muted);
}
.account-options :deep(.test-users) {
  margin-top: 16px;
  border: 0;
  padding: 0;
}
.ai-settings :deep(.connections) {
  border: 0;
  padding: 0;
  margin: 0;
}
.ai-settings :deep(fieldset) {
  min-width: 0;
  border: 0;
  padding: 0;
  margin: 0;
}
.ai-settings :deep(.connections h3) {
  font-size: 13px;
  margin: 0 0 12px;
}
.ai-settings :deep(.connections p) {
  font-size: 11px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
}
.ai-settings :deep(.connections ul) {
  list-style: none;
  padding: 0;
  margin: 18px 0;
}
.ai-settings :deep(.connections li) {
  border: 1px solid var(--rss-color-border);
  border-radius: 9px;
  padding: 14px;
  margin-bottom: 10px;
  font-size: 12px;
  line-height: 2;
}
.ai-settings :deep(button) {
  min-height: 32px;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  padding: 6px 10px;
  font-size: 12px;
  color: var(--rss-color-text);
  background: var(--rss-color-surface);
}
.ai-settings :deep(.connection-form) {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  padding-top: 20px;
  border-top: 1px solid var(--rss-color-border);
}
.ai-settings :deep(.connection-form > :is(h3, p, details, div)) {
  grid-column: 1/-1;
  margin: 0;
}
.ai-settings :deep(label) {
  display: grid;
  gap: 8px;
  min-width: 0;
  font-size: 12px;
}
.ai-settings :deep(input),
.ai-settings :deep(select) {
  min-width: 0;
  max-width: 100%;
  min-height: 38px;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  padding: 8px 10px;
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
  font-size: 12px;
}
.ai-settings :deep(.session-diagnostics > h3) {
  display: none;
}
.about-product {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 22px;
}
.agent-mark {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border-radius: 11px;
  background: var(--rss-color-text);
  color: var(--rss-color-surface);
  font-size: 26px;
  letter-spacing: -2px;
  font-weight: 600;
}
.about-product strong {
  font-size: 14px;
}
.about-product p {
  font-size: 11px;
  margin: 5px 0 0;
  color: var(--rss-color-text-muted);
}
.privacy-rows {
  margin: 0;
}
.privacy-rows > div {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr);
  gap: 16px;
  padding: 14px 0;
  border-top: 1px solid var(--rss-color-border);
  font-size: 12px;
  line-height: 1.8;
}
.settings-confirm {
  position: fixed;
  inset: 20% max(16px, calc((100vw - 520px) / 2)) auto;
  z-index: 150;
  padding: 24px;
  border: 1px solid var(--rss-color-border-strong);
  border-radius: 14px;
  background: var(--rss-color-surface);
  box-shadow: 0 0 0 100vmax #0006;
}
.settings-confirm h2 {
  font-size: 18px;
  margin-top: 0;
}
.settings-confirm p {
  font-size: 13px;
  line-height: 1.8;
}
@media (max-width: 700px) {
  .settings.settings-page {
    padding: 24px 18px 40px;
  }
  .settings-surface {
    padding: 18px 16px;
  }
  .settings-facts > div,
  .privacy-rows > div {
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 10px;
  }
  .ai-settings :deep(.connection-form) {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
