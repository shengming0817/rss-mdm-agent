<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from "vue";
import { currentUser } from "./test-users";
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
  assistantServices?: AssistantServices;
  page: string;
  host: HostSettings;
}>();
const emit = defineEmits<{
  navigate: [id: string];
  attention: [count: number];
  mode: [label: string];
}>();
const newIdentity = () => crypto.randomUUID();
const enterprise = currentUser.value?.identity?.mode === "enterprise";
const controller = createController(
  enterprise ? null : nativePort(),
  newIdentity,
  preview,
);
const assistant = createAssistant(
  props.assistantServices ?? nativeAssistant(),
  newIdentity,
);
watch(
  () => props.page,
  (id) => {
    if (id !== "assistant" && id !== "settings") controller.navigate(id);
  },
  { immediate: true },
);
watch(assistant.attention, (count) => emit("attention", count), {
  immediate: true,
});
watch(
  () => [props.page, assistant.state.connection, assistant.state.mode],
  () =>
    emit(
      "mode",
      props.page === "assistant"
        ? assistant.state.connection !== "connected"
          ? "AI 服务未连接"
          : assistant.state.mode === "s1"
            ? "S1 AI 测试装配 · 无真实执行"
            : "AI 会话"
        : controller.interactive
          ? "S1 受控测试 · 无真实执行"
          : "浏览器只读预览",
    ),
  { immediate: true },
);
onMounted(() => {
  void assistant.connect();
});
onBeforeUnmount(assistant.dispose);
</script>
<template>
  <section
    v-if="enterprise && page !== 'assistant' && page !== 'settings'"
    aria-label="企业能力说明"
  >
    <h1>企业账户</h1>
    <p>
      已登录企业账户，可使用独立的个人 AI 工作区。企业设备执行与批准尚未接线。
    </p>
    <button @click="emit('navigate', 'assistant')">前往 AI 助手</button>
    <button @click="emit('navigate', 'settings')">
      切换测试用户或不登录使用
    </button>
  </section>
  <SelfService
    v-if="!enterprise"
    v-show="page !== 'assistant' && page !== 'settings'"
    :controller="controller"
  />
  <Assistant
    v-if="page === 'assistant'"
    :controller="assistant"
    @settings="emit('navigate', 'settings')"
    @tasks="emit('navigate', 'tasks')"
  />
  <Settings
    v-show="page === 'settings'"
    :host="host"
    :active="page === 'settings'"
    :assistant="assistant"
    @assistant="emit('navigate', 'assistant')"
    ><template #user><slot name="user" /></template
  ></Settings>
</template>
