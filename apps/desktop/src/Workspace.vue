<script setup lang="ts">
import {
  computed,
  nextTick,
  ref,
  shallowRef,
  onBeforeUnmount,
  onMounted,
  watch,
} from "vue";
import {
  AppShell,
  NavigationList,
  Sparkles,
  UserRound,
  ModalDrawer,
} from "@rss-mdm-agent/ui";
import ConversationList from "./assistant/ConversationList.vue";
import { nativeAssistant } from "./assistant/native";
import Assistant from "./assistant/Assistant.vue";
import ContextPanel from "./assistant/ContextPanel.vue";
import {
  resourceContext,
  contextCurrent,
  type ResourceContext,
} from "./assistant/resource-context";
import type { BackendTask, SelfServicePort } from "./self-service/types";
import {
  createAssistant,
  type AssistantServices,
} from "./assistant/controller";
import SelfService from "./self-service/SelfService.vue";
import { createController } from "./self-service/controller";
import { nativePort } from "./self-service/native";
import preview from "./self-service/preview";
import Settings from "./settings/Settings.vue";
import type { ServicePort } from "./settings/native";
import type { HostSettings } from "./settings/controller";
import "./self-service/style.css";
import "./assistant/style.css";
const props = defineProps<{
  ready: boolean;
  busy: boolean;
  assistantServices?: AssistantServices;
  selfServicePort?: SelfServicePort;
  page: string;
  host: HostSettings;
  servicePort?: ServicePort;
  fixture?: boolean;
}>();
const emit = defineEmits<{
  navigate: [id: string];
}>();
const newIdentity = () => crypto.randomUUID();
const controller = props.ready
  ? createController(props.selfServicePort ?? nativePort(), preview)
  : undefined;
const assistant = props.ready
  ? createAssistant(props.assistantServices ?? nativeAssistant(), newIdentity)
  : undefined;
watch(
  () => props.page,
  (id) => {
    if (id !== "assistant" && id !== "settings") controller?.navigate(id);
  },
  { immediate: true },
);
const contextOpen = ref(false),
  wide = ref(false),
  chosen = ref(false),
  moving = ref(false);
const candidate = shallowRef<ResourceContext>(),
  target = shallowRef<HTMLElement>();
const panelHost = ref<HTMLElement>();
const headerHost = ref<HTMLElement>();
const contextSelection = ref("");
const panelError = ref(""),
  panelNotice = ref("");
let transition = 0;
let trigger: HTMLElement | null = null;
const assistantVisible = computed(
  () =>
    !moving.value &&
    (props.page === "assistant" || (contextOpen.value && chosen.value)),
);
async function relocate(change: () => void) {
  const version = ++transition;
  moving.value = true;
  target.value = undefined;
  await nextTick();
  if (version !== transition) return;
  change();
  await nextTick();
  if (version !== transition) return;
  if (contextOpen.value && chosen.value) target.value = panelHost.value;
  moving.value = false;
}
async function askAi(item: BackendTask, source: HTMLElement) {
  const snapshot = controller?.state.snapshot;
  if (!snapshot || !assistant) return;
  const value = resourceContext(item, assistant.state.mode);
  if (
    !value ||
    !contextCurrent(value, snapshot.available, assistant.state.mode)
  )
    return;
  trigger = source;
  await relocate(() => {
    candidate.value = value;
    contextSelection.value = "";
    chosen.value = false;
    panelError.value = "";
    panelNotice.value = "";
    contextOpen.value = true;
  });
}
async function chooseConversation(id: string) {
  const value = candidate.value,
    version = transition;
  if (!assistant || !value || value.stale) return;
  if (id !== "new" && !assistant.state.sessions.has(id)) {
    panelError.value = "会话不可用，请重新选择。";
    return;
  }
  if (id === "new") assistant.create();
  else await assistant.select(id);
  if (
    version !== transition ||
    !contextOpen.value ||
    candidate.value !== value ||
    value.stale
  )
    return;
  if (id !== "new" && assistant.state.selected !== id) return;
  panelNotice.value =
    assistant.context.value && assistant.context.value.text !== value.text
      ? `已替换待发送资源为“${value.path[2]}”，原问题已保留。`
      : "";
  assistant.stageContext(value);
  await relocate(() => {
    chosen.value = true;
  });
  await nextTick();
  panelHost.value?.querySelector<HTMLTextAreaElement>("textarea")?.focus();
}
async function closeContext() {
  await relocate(() => {
    contextOpen.value = false;
    candidate.value = undefined;
    chosen.value = false;
  });
  if (trigger?.isConnected && trigger.getClientRects().length) trigger.focus();
}
async function mainConversation() {
  await closeContext();
  emit("navigate", "assistant");
}
watch(
  () => props.page,
  () => {
    if (contextOpen.value) void closeContext();
  },
);
watch(
  () => controller?.state.snapshot,
  (snapshot) => {
    if (!snapshot) return;
    assistant?.validateContexts(
      snapshot.available,
      assistant?.state.mode ?? "live",
    );
    if (
      candidate.value &&
      !contextCurrent(
        candidate.value,
        snapshot.available,
        assistant?.state.mode ?? "live",
      )
    )
      candidate.value = Object.freeze({ ...candidate.value, stale: true });
  },
  { deep: true },
);
let sizing: ResizeObserver | undefined;
const workspaceRoot = ref<HTMLElement>();
const attention = computed(() => assistant?.attention.value ?? 0);
const mode = computed(() =>
  !props.ready
    ? "选择使用身份"
    : assistant?.state.mode === "s1"
      ? "S1 受控测试 · 无真实执行"
      : controller?.interactive
        ? assistant?.state.connection === "connected"
          ? "后台任务 · 本机执行服务"
          : "AI 服务未连接 · 本机执行服务"
        : "浏览器只读预览",
);
onMounted(() => {
  void assistant?.connect();
  if (workspaceRoot.value) {
    const resize = () => {
      const next = workspaceRoot.value!.getBoundingClientRect().width >= 1200;
      if (wide.value !== next) {
        wide.value = next;
        if (contextOpen.value && chosen.value) void relocate(() => {});
      }
    };
    resize();
    sizing = new ResizeObserver(resize);
    sizing.observe(workspaceRoot.value);
  }
});
onBeforeUnmount(() => {
  transition++;
  sizing?.disconnect();
  assistant?.dispose();
});
</script>
<template>
  <AppShell
    :navigation-enabled="ready"
    :navigation-key="page"
    :content-mode="ready && page !== 'settings' ? 'conversation' : 'page'"
  >
    <template #navigation-brand>
      <div class="workspace-brand">
        <span class="brand-mark"
          ><Sparkles :size="20" aria-hidden="true" /></span
        ><strong>RSS</strong>
      </div>
      <p class="workspace-description">你的 AI 与设备工作区</p>
      <p class="workspace-mode">{{ mode }}</p>
    </template>
    <template #header>
      <div class="workspace-page-header">
        <div
          ref="headerHost"
          class="assistant-header-host"
          :hidden="page !== 'assistant'"
        />
        <h1 v-if="page !== 'assistant'" tabindex="-1">
          {{
            !ready || page === "settings"
              ? "设置"
              : page === "tasks"
                ? "请求与任务"
                : page === "software"
                  ? "软件中心"
                  : page === "tools"
                    ? "工具中心"
                    : page === "help"
                      ? "设备与帮助"
                      : "首页"
          }}
        </h1>
      </div>
    </template>
    <template #navigation="{ navigate }"
      ><NavigationList
        :items="[
          {
            id: 'assistant',
            label: 'AI 助手',
            icon: 'assistant',
            badge: attention,
          },
          { id: 'home', icon: 'home', label: '首页' },
          { id: 'software', icon: 'software', label: '软件中心' },
          { id: 'tools', icon: 'tools', label: '工具中心' },
          { id: 'tasks', icon: 'tasks', label: '请求与任务' },
          {
            id: 'help',
            icon: 'device',
            label: '设备与帮助 · 未接线',
            disabled: true,
          },
        ]"
        :active-id="page"
        @select="
          (id) => {
            emit('navigate', id);
            navigate();
          }
        "
    /></template>
    <template #conversations="{ navigate }"
      ><ConversationList
        v-if="assistant"
        :controller="assistant"
        @select="
          emit('navigate', 'assistant');
          navigate();
        "
    /></template>
    <template #navigation-footer="{ navigate }"
      ><NavigationList
        :items="[{ id: 'settings', icon: 'settings', label: '设置' }]"
        :active-id="page"
        @select="
          (id) => {
            emit('navigate', id);
            navigate();
          }
        "
      />
      <button
        class="workspace-account"
        type="button"
        @click="
          emit('navigate', 'settings');
          navigate();
        "
      >
        <span class="account-avatar"
          ><UserRound :size="18" aria-hidden="true"
        /></span>
        <span><slot name="account-summary">账户与连接</slot></span>
      </button>
    </template>
    <div
      ref="workspaceRoot"
      class="workspace-content"
      :class="{
        'context-layout': contextOpen && wide,
      }"
      :inert="busy ? true : undefined"
    >
      <div
        class="resource-content"
        v-if="controller"
        v-show="page !== 'assistant' && page !== 'settings'"
      >
        <SelfService
          v-if="controller"
          v-show="page !== 'assistant' && page !== 'settings'"
          :controller="controller"
          :context-key="candidate?.key ?? assistant?.context.value?.key"
          :source="
            assistant?.state.mode === 's1'
              ? 's1'
              : controller.interactive
                ? 'live'
                : 'preview'
          "
          @ask-ai="askAi"
        />
      </div>
      <Teleport :to="target ?? 'body'" :disabled="!target"
        ><Assistant
          v-if="assistant"
          :visible="assistantVisible"
          :controller="assistant"
          :presentation="contextOpen && chosen ? 'context' : 'main'"
          :header-target="headerHost"
          :portal-target="target ?? headerHost"
          @settings="emit('navigate', 'settings')"
          @tasks="emit('navigate', 'tasks')"
      /></Teleport>
      <ModalDrawer
        v-if="contextOpen && !wide && candidate && assistant"
        class="resource-context-drawer"
        label="资源上下文 AI"
        side="right"
        @close="closeContext"
      >
        <ContextPanel
          v-model:selection="contextSelection"
          :wide="false"
          :candidate="candidate"
          :controller="assistant"
          :chosen="chosen"
          :notice="panelNotice"
          :error="
            panelError ||
            (candidate.stale && !chosen
              ? '资源信息已变更，请从最新资源详情重新选择。'
              : '')
          "
          @choose="chooseConversation"
          @close="closeContext"
          @main="mainConversation"
          ><div ref="panelHost" class="assistant-panel-host"
        /></ContextPanel>
      </ModalDrawer>
      <aside
        v-if="contextOpen && wide && candidate && assistant"
        class="resource-context-aside"
        aria-label="资源上下文 AI"
      >
        <ContextPanel
          v-model:selection="contextSelection"
          :wide="true"
          :candidate="candidate"
          :controller="assistant"
          :chosen="chosen"
          :notice="panelNotice"
          :error="
            panelError ||
            (candidate.stale && !chosen
              ? '资源信息已变更，请从最新资源详情重新选择。'
              : '')
          "
          @choose="chooseConversation"
          @close="closeContext"
          @main="mainConversation"
          ><div ref="panelHost" class="assistant-panel-host"
        /></ContextPanel>
      </aside>
      <Settings
        v-show="!ready || page === 'settings'"
        :host="host"
        :service-port="servicePort"
        :fixture="fixture"
        :active="!ready || page === 'settings'"
        :assistant="assistant"
        @assistant="emit('navigate', 'assistant')"
        ><template #user><slot name="user" /></template
      ></Settings>
    </div>
    <template #status
      ><span v-if="busy" role="status">正在读取或切换账户…</span
      ><span v-else
        >会话保存在本机 · 模型请求发送至所选服务 · 设备状态独立核实</span
      ></template
    >
  </AppShell>
</template>
<style scoped>
.workspace-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}
.workspace-brand strong {
  font-size: 22px;
  letter-spacing: -0.5px;
}
.brand-mark {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: var(--rss-color-text);
  color: var(--rss-color-bg);
}
.workspace-description {
  margin: 10px 0 0;
  color: var(--rss-color-text-muted);
  font-size: var(--rss-font-size-xs);
}
.workspace-mode {
  margin: 8px 0 0;
  font-size: var(--rss-font-size-xs);
  color: var(--rss-color-text-muted);
  line-height: 1.5;
}
.workspace-page-header {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
}
.assistant-header-host {
  flex: 1;
  min-width: 0;
}
.assistant-header-host[hidden] {
  display: none;
}
.workspace-page-header h1 {
  margin: 0;
  font-size: var(--rss-font-size-md);
  font-weight: 500;
}
.workspace-account {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  padding: 10px 12px;
  text-align: left;
  border: 0;
  background: transparent;
}
.workspace-account > span:last-child {
  min-width: 0;
  overflow-wrap: anywhere;
}
.account-avatar {
  display: grid;
  place-items: center;
  flex: none;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: var(--rss-color-accent-bg);
  color: var(--rss-color-accent);
}
.workspace-content {
  height: 100%;
  min-height: 0;
}
.resource-content {
  height: 100%;
  min-width: 0;
  overflow: auto;
  padding: 24px;
}
.context-layout {
  display: flex;
}
.context-layout > .resource-content {
  flex: 1;
}
.assistant-panel-host {
  height: 100%;
  min-height: 0;
}
.resource-context-drawer {
  --rss-drawer-width: 520px;
  --rss-drawer-padding: 16px;
}
.resource-context-drawer[open] {
  display: flex;
  flex-direction: column;
}
.resource-context-drawer > :deep(.context-panel) {
  flex: 1;
  min-height: 0;
}
.resource-context-aside {
  min-width: 0;
  flex: 0 0 var(--rss-inspector-width);
  min-height: 0;
}
@media (max-width: 640px) {
  .resource-content {
    padding: 16px;
  }
}
</style>
