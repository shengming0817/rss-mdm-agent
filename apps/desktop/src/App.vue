<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { AppShell, NavigationList } from "@rss-mdm-agent/ui";
import Assistant from "./assistant/Assistant.vue";
import {
  createAssistant,
  type AssistantServices,
} from "./assistant/controller";
import SelfService from "./self-service/SelfService.vue";
import { createController } from "./self-service/controller";
import { nativePort } from "./self-service/native";
import preview from "./self-service/preview";
import "./self-service/style.css";
import "./assistant/style.css";
const props = defineProps<{ assistantServices?: AssistantServices }>();
const newIdentity = () => crypto.randomUUID();
const controller = createController(nativePort(), newIdentity, preview);
const assistant = createAssistant(props.assistantServices, newIdentity);
const page = ref("self-service");
const attention = assistant.attention;
function navigate(id: string) {
  page.value = id === "assistant" ? "assistant" : "self-service";
  if (page.value === "self-service") controller.navigate(id);
}
onMounted(() => {
  void assistant.connect();
});
onBeforeUnmount(assistant.dispose);
</script>
<template>
  <AppShell>
    <template #header
      ><div class="brand">
        <div>
          <span class="eyebrow">RSS / WORKSPACE</span
          ><strong>自助服务中心</strong>
        </div>
        <span class="mode-label">{{
          page === "assistant"
            ? assistant.state.mode === "s1"
              ? "S1 AI 测试装配 · 无真实执行"
              : assistant.state.connection === "connected"
                ? "AI 会话"
                : "AI 服务未连接"
            : controller.interactive
              ? "固定测试服务 · 无真实执行"
              : "浏览器只读预览"
        }}</span>
      </div></template
    >
    <template #navigation
      ><NavigationList
        :items="[
          { id: 'home', label: '首页' },
          { id: 'software', label: '软件中心' },
          { id: 'tools', label: '工具中心' },
          { id: 'tasks', label: '请求与任务' },
          {
            id: 'assistant',
            label: attention ? `AI 助手（待回应 ${attention}）` : 'AI 助手',
          },
          { id: 'help', label: '设备与帮助' },
        ]"
        :active-id="page === 'assistant' ? 'assistant' : controller.state.page"
        @select="navigate"
    /></template>
    <SelfService v-show="page === 'self-service'" :controller="controller" />
    <Assistant v-show="page === 'assistant'" :controller="assistant" />
    <template #status
      ><div class="footer-note">
        <span>测试服务 · 无系统副作用 · 无可信批准</span
        ><span>AI 对话与设备执行分别核对</span>
      </div></template
    >
  </AppShell>
</template>
