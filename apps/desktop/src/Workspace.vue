<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from "vue";
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
const controller = createController(nativePort(), newIdentity, preview);
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
  <SelfService
    v-show="page !== 'assistant' && page !== 'settings'"
    :controller="controller"
  />
  <Assistant
    v-show="page === 'assistant'"
    :controller="assistant"
    @settings="emit('navigate', 'settings')"
  />
  <Settings
    v-show="page === 'settings'"
    :host="host"
    :assistant="assistant"
    @assistant="emit('navigate', 'assistant')"
    ><template #user><slot name="user" /></template
  ></Settings>
</template>
