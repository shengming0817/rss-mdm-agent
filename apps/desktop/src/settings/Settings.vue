<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { AssistantController } from "../assistant/controller";
import LocalService from "./LocalService.vue";
import ConnectionSettings from "./ConnectionSettings.vue";
import { diagnosticMessage, type HostSettings } from "./controller";
const props = defineProps<{
  host: HostSettings;
  assistant?: AssistantController;
}>();
const emit = defineEmits<{ assistant: []; reconnected: [] }>();
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
async function newConversation() {
  const c = props.assistant;
  if (!c) return;
  const previous = c.state.selected;
  await c.create();
  if (c.state.selected && c.state.selected !== previous) emit("assistant");
}
async function confirm() {
  confirming.value = true;
  await nextTick();
  dialog.value?.querySelector<HTMLElement>("button")?.focus();
}
async function close() {
  confirming.value = false;
  await nextTick();
  trigger.value?.focus();
}
async function restart() {
  await close();
  if (await props.host.restart()) {
    await props.assistant?.connect();
    emit("reconnected");
  }
  await nextTick();
  trigger.value?.focus();
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
  if (!buttons?.length) return;
  const first = buttons[0],
    last = buttons.at(-1)!;
  if (event.shiftKey && event.target === first) {
    event.preventDefault();
    last.focus();
  }
  if (!event.shiftKey && event.target === last) {
    event.preventDefault();
    first.focus();
  }
}
</script>
<template>
  <section class="settings" aria-label="设置">
    <h1 tabindex="-1">设置</h1>
    <div :inert="confirming ? true : undefined">
      <LocalService />
      <section class="settings-card">
        <h2>测试用户</h2>
        <p>本地测试模式 · 非登录 · 不代表企业身份认证。</p>
        <slot name="user" />
      </section>
      <section class="settings-card">
        <h2>AI 连接</h2>
        <template v-if="assistant">
          <p v-if="!readyConnection">
            首次配置：选择测试用户 → 验证并保存连接 → 新建对话。
          </p>
          <ConnectionSettings :controller="assistant" />
          <button
            :disabled="
              !readyConnection ||
              assistant.state.connection !== 'connected' ||
              assistant.state.opening
            "
            @click="newConversation"
          >
            新建对话并前往 AI
          </button>
          <button @click="emit('assistant')">稍后配置，前往 AI</button>
          <p v-if="assistant.state.createError" role="alert">
            新建会话未完成，请重新连接后重试。
          </p>
        </template>
        <p v-else>
          先选择测试用户，再配置该用户的个人连接。没有凭据也可使用自助入口。
        </p>
      </section>
      <section class="settings-card">
        <h2>常规与通知</h2>
        <dl>
          <dt>外观</dt>
          <dd>跟随系统</dd>
          <dt>关闭窗口</dt>
          <dd>
            关闭视图后 AI Host
            继续运行；应用菜单“退出”关闭应用进程。独立状态服务由系统管理。
          </dd>
          <dt>通知</dt>
          <dd>应用内显示待回应计数；当前未提供系统通知和自动更新设置。</dd>
        </dl>
      </section>
      <section class="settings-card">
        <h2>数据与诊断</h2>
        <p>
          AI Host：{{
            host.state.status
              ? phases.get(host.state.status.phase)
              : host.available
                ? "正在读取状态"
                : "浏览器预览不可用"
          }}
        </p>
        <p v-if="host.state.status">
          运行包来源：{{
            host.state.status.source === "development_override"
              ? "显式开发路径"
              : "应用内资源"
          }}
        </p>
        <p v-if="diagnosticMessage(host.state.status)" role="alert">
          {{ diagnosticMessage(host.state.status) }}
        </p>
        <p>
          断线期间仅保留当前用户已加载的授权历史，恢复后重新核对。切换用户清空视图；设备任务保持原归属。
        </p>
        <button
          :disabled="!host.available || host.state.busy || host.state.loading"
          @click="host.refresh"
        >
          刷新状态
        </button>
        <button
          ref="trigger"
          :disabled="!host.available || !host.state.status || host.state.busy"
          @click="confirm"
        >
          重启 AI Host
        </button>
        <button
          :disabled="!host.available || host.state.busy"
          @click="host.exportDiagnostics"
        >
          导出脱敏诊断
        </button>
        <button
          v-if="assistant"
          :disabled="
            assistant.state.connection === 'connecting' || host.state.busy
          "
          @click="assistant.connect"
        >
          重新连接
        </button>
        <p v-if="host.state.busy" role="status">正在处理，请稍候…</p>
        <p v-if="host.state.readError" role="alert">
          {{ host.state.readError }}
        </p>
        <p v-if="host.state.message" role="status">{{ host.state.message }}</p>
        <p>
          导出只包含版本、状态、错误码和时间，不包含密钥、对话、端点或个人路径。
        </p>
      </section>
      <section class="settings-card">
        <details>
          <summary>关于</summary>
          <p>
            RSS MDM Agent · {{ host.state.status?.version ?? "浏览器预览" }}
          </p>
          <p>MIT 许可。S1 测试装配，无真实系统执行。</p>
          <h3>隐私</h3>
          <p>
            用户、连接和历史保存在本机。API
            密钥在原生输入框提交后加密保存；官方工具持有自己的登录配置。对话发送至所选服务，跨连接携带历史需要明确确认。诊断由用户主动导出。
          </p>
        </details>
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
      <button @click="close">返回设置</button
      ><button @click="restart">确认重启</button>
    </section>
  </section>
</template>
<style scoped>
.settings {
  max-width: 1064px;
  margin: 0 auto;
}
.settings-card {
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-surface);
  padding: 20px;
  margin: 16px 0;
  overflow-wrap: anywhere;
}
.settings-card h2 {
  margin-top: 0;
}
.settings-card p,
dd {
  line-height: 1.6;
}
button {
  padding: 8px 12px;
  margin: 4px;
}
dd {
  margin: 0 0 12px;
}
dt {
  font-weight: 600;
}
.settings-confirm {
  position: fixed;
  inset: 20% max(16px, calc((100vw - 520px) / 2)) auto;
  background: var(--color-surface);
  border: 2px solid var(--color-border);
  box-shadow: 0 0 0 100vmax #0006;
  padding: 24px;
  z-index: 10;
}
@media (max-width: 640px) {
  .settings-card {
    padding: 12px;
  }
}
</style>
