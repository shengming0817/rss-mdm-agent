<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch } from "vue";
import { AppShell, NavigationList, Sparkles } from "@rss-mdm-agent/ui";
import ConversationList from "./assistant/ConversationList.vue";
import { nativeAssistant } from "./assistant/native";
import Assistant from "./assistant/Assistant.vue";
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
  page: string;
  host: HostSettings;
}>();
const emit = defineEmits<{
  navigate: [id: string];
}>();
const newIdentity = () => crypto.randomUUID();
const controller = props.ready
  ? createController(nativePort(), preview)
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
});
onBeforeUnmount(() => assistant?.dispose());
</script>
<template>
  <AppShell
    :navigation-enabled="ready"
    :navigation-key="page"
    :content-mode="ready && page === 'assistant' ? 'conversation' : 'page'"
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
      class="workspace-content"
      :class="{ 'conversation-content': ready && page === 'assistant' }"
      :inert="busy ? true : undefined"
    >
      <SelfService
        v-if="controller"
        v-show="page !== 'assistant' && page !== 'settings'"
        :controller="controller"
      />
      <Assistant
        v-if="assistant"
        v-show="page === 'assistant'"
        :visible="page === 'assistant'"
        :controller="assistant"
        @settings="emit('navigate', 'settings')"
        @tasks="emit('navigate', 'tasks')"
      />
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
