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
      const next = window.innerWidth >= 1440;
      if (wide.value !== next)
        void relocate(() => {
          wide.value = next;
        });
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
    :content-mode="
      ready && (page === 'assistant' || (contextOpen && wide))
        ? 'conversation'
        : 'page'
    "
  >
    <template #header
      ><div class="workspace-brand">
        <Sparkles :size="22" aria-hidden="true" /><strong>RSS 工作区</strong
        ><span class="workspace-mode">{{ mode }}</span>
      </div></template
    >
    <template #navigation="{ navigate }"
      ><NavigationList
        :items="[
          {
            id: 'assistant',
            label: attention ? `AI 助手（待回应 ${attention}）` : 'AI 助手',
          },
          { id: 'home', label: '首页' },
          { id: 'software', label: '软件中心' },
          { id: 'tools', label: '工具中心' },
          { id: 'tasks', label: '请求与任务' },
          { id: 'help', label: '设备与帮助' },
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
        :items="[{ id: 'settings', label: '设置' }]"
        :active-id="page"
        @select="
          (id) => {
            emit('navigate', id);
            navigate();
          }
        "
    /></template>
    <div
      ref="workspaceRoot"
      class="workspace-content"
      :class="{
        'conversation-content': ready && page === 'assistant',
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
          @ask-ai="askAi"
        />
      </div>
      <Teleport :to="target ?? 'body'" :disabled="!target"
        ><Assistant
          v-if="assistant"
          v-show="assistantVisible"
          :visible="assistantVisible"
          :controller="assistant"
          :portal-target="target"
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
        :active="!ready || page === 'settings'"
        :assistant="assistant"
        @assistant="emit('navigate', 'assistant')"
        ><template #user><slot name="user" /></template
      ></Settings>
    </div>
    <template #status
      ><span v-if="busy" role="status">正在读取或切换账户…</span
      ><span v-else>后台授权任务 · 状态与效果分别核实</span></template
    >
  </AppShell>
</template>
<style scoped>
.workspace-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}
.workspace-brand strong {
  white-space: nowrap;
  font-size: 15px;
}
.workspace-mode {
  margin-left: auto;
  color: var(--rss-color-text-muted);
  font-size: var(--rss-font-size-xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.workspace-content.conversation-content {
  height: 100%;
  min-height: 0;
}
</style>

<style scoped>
.context-layout {
  display: flex;
  height: 100%;
  min-height: 0;
}
.context-layout > .resource-content {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 24px;
}
.assistant-panel-host {
  height: 100%;
  min-height: 0;
}
</style>

<style scoped>
.resource-context-drawer {
  --rss-drawer-width: 600px;
  --rss-drawer-padding: 12px;
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
  flex: 0 0 560px;
  min-height: 0;
}
</style>
