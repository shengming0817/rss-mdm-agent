<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { LocalSoftwareStatus, SoftwarePort } from "../self-service/types";
import { nativeSoftwarePort } from "../self-service/native";
import { currentUser } from "../test-users";
import { ended } from "./controller";
const props = withDefaults(
  defineProps<{
    request: string;
    port?: SoftwarePort | null;
    visible?: boolean;
  }>(),
  { visible: true },
);
const emit = defineEmits<{ changed: [value: LocalSoftwareStatus] }>();
const port = props.port === undefined ? nativeSoftwarePort() : props.port;
const value = ref<LocalSoftwareStatus>(),
  busy = ref(false),
  error = ref(""),
  uncertain = ref(false),
  now = ref(Date.now());
let active = true,
  epoch = 0;
const actionLabels = {
  launch: "打开",
  update: "更新",
  uninstall: "卸载",
  detect: "检测",
};
const phaseLabels: Record<string, string> = {
  waiting: "已准备，尚未提交",
  confirmationRequired: "等待本人确认",
  accepted: "已接收，启动状态待核实",
  running: "正在执行",
  outcomeUnknown: "执行效果未知",
  executionEnded: "执行已结束，请核对检测结果",
  verified: "已有结果核实",
  cancelled: "已取消",
  admissionDenied: "准入被拒绝",
  failedBeforeDispatch: "启动前失败",
};
const canSubmit = computed(
  () =>
    value.value &&
    !uncertain.value &&
    !busy.value &&
    value.value.execution.attempts === 0 &&
    !value.value.execution.cancelRequested &&
    ["waiting", "confirmationRequired"].includes(value.value.execution.phase) &&
    now.value < value.value.prepared.startBeforeUnixMs,
);
async function refresh() {
  if (!port || !active || busy.value || !props.visible) return;
  const call = epoch;
  busy.value = true;
  try {
    const result = await port.status({ request: props.request });
    if (!active || call !== epoch) return;
    value.value = result;
    uncertain.value = false;
    error.value = "";
    emit("changed", result);
  } catch {
    if (active && call === epoch) {
      uncertain.value = true;
      error.value = "原请求状态暂时无法核实；请继续查询。";
    }
  } finally {
    if (active && call === epoch) busy.value = false;
  }
}
async function mutate(kind: "submit" | "cancel") {
  if (
    !value.value ||
    !port ||
    busy.value ||
    (kind === "submit" && !canSubmit.value)
  )
    return;
  const call = epoch;
  busy.value = true;
  try {
    const result = await (kind === "submit"
      ? port.submit({
          request: props.request,
          digest: value.value.prepared.digest,
        })
      : port.cancel({ request: props.request }));
    if (!active || call !== epoch) return;
    value.value = result;
    uncertain.value = false;
    error.value = "";
    emit("changed", result);
  } catch {
    if (active && call === epoch) {
      uncertain.value = true;
      error.value = "操作回执未确认；请查询原请求，避免重复操作。";
    }
  } finally {
    if (active && call === epoch) busy.value = false;
  }
}
watch(
  () => [props.request, props.visible],
  () => {
    if (props.visible) void refresh();
  },
  { immediate: true },
);
watch(
  () => currentUser.value?.generation,
  () => {
    ++epoch;
    active = false;
    value.value = undefined;
    error.value = "用户已切换，请在当前会话重新读取。";
  },
);
const polling = setInterval(() => {
  now.value = Date.now();
  if (props.visible && (!value.value || !ended(value.value))) void refresh();
}, 2000);
onBeforeUnmount(() => {
  active = false;
  ++epoch;
  clearInterval(polling);
});
</script>
<template>
  <section class="local-software-action" aria-label="本地软件原请求">
    <p v-if="error" role="alert">{{ error }}</p>
    <template v-if="value">
      <p v-if="value.execution.mode === 'test'">
        测试执行记录，不代表真实设备效果。
      </p>
      <h3>
        {{ actionLabels[value.prepared.choice.action] }} ·
        {{ value.prepared.name }}
      </h3>
      <dl class="facts">
        <dt>执行范围</dt>
        <dd>
          {{
            value.prepared.identity === "system"
              ? "设备级 · 系统服务"
              : "当前用户 · 登录会话"
          }}
        </dd>
        <dt>发起来源</dt>
        <dd>
          {{
            value.prepared.initiator.kind === "ai"
              ? "AI 发起，本人确认后提交"
              : "本人发起"
          }}
        </dd>
        <template
          v-if="
            value.prepared.initiator.kind === 'human' ||
            value.prepared.initiator.kind === 'ai'
          "
        >
          <dt>原 OS 用户 / 会话</dt>
          <dd>
            {{ value.prepared.initiator.osSession.account.subject }} /
            {{ value.prepared.initiator.osSession.session }}
          </dd>
        </template>
        <template v-if="value.prepared.initiator.kind === 'ai'"
          ><dt>原 AI 会话 / 调用</dt>
          <dd>
            {{ value.prepared.initiator.conversation }} /
            {{ value.prepared.initiator.toolCall }}
          </dd></template
        >
        <dt>原请求</dt>
        <dd class="identifier">{{ request }}</dd>
        <dt>状态</dt>
        <dd>
          {{
            uncertain
              ? "状态待核实"
              : (phaseLabels[value.execution.phase] ?? value.execution.phase)
          }}
        </dd>
        <dt v-if="value.prepared.targetVersion">目标版本</dt>
        <dd v-if="value.prepared.targetVersion">
          {{ value.prepared.targetVersion }}
        </dd>
      </dl>
      <p
        v-if="
          !value.execution.attempts && now >= value.prepared.startBeforeUnixMs
        "
      >
        准备已过期，不能提交；原记录仍可查询。
      </p>
      <p v-if="value.detected?.kind === 'absent'">
        原生检测：所选安装目标已不存在。
      </p>
      <p v-else-if="value.detected?.kind === 'present'">
        原生检测：目标存在，版本 {{ value.detected.version }}。
      </p>
      <p v-else-if="value.execution.software === 'desiredStateObserved'">
        独立检测已观察到目标状态。
      </p>
      <p
        v-else-if="
          value.execution.attempts && value.prepared.choice.action !== 'launch'
        "
      >
        安装状态尚待检测核实。
      </p>
      <p v-if="value.execution.software === 'restartPending'">
        等待系统重启后核实；此操作不会自动重启。
      </p>
      <p
        v-if="
          value.prepared.choice.action === 'launch' &&
          value.execution.process?.exitCode === 0 &&
          value.execution.process.quiescent
        "
      >
        系统启动接口已返回应用实例；窗口状态以当前桌面为准。
      </p>
      <p v-if="value.execution.process">
        进程：{{ value.execution.process.finished ? "已退出" : "运行中" }} ·
        退出码 {{ value.execution.process.exitCode ?? "未知" }} ·
        {{
          value.execution.process.quiescent
            ? "执行范围已静止"
            : "尚未证明全部停止"
        }}
      </p>
      <p v-if="value.execution.cancelRequested">
        已请求取消；这不代表进程已停止或变更已回滚。
      </p>
      <button
        v-if="
          value.execution.attempts === 0 && !value.execution.cancelRequested
        "
        :disabled="!canSubmit"
        @click="mutate('submit')"
      >
        确认提交原操作
      </button>
      <button
        :disabled="busy || value.execution.cancelRequested || ended(value)"
        @click="mutate('cancel')"
      >
        请求取消原操作
      </button>
    </template>
    <p v-else>正在读取原软件请求。</p>
    <button :disabled="busy || !port" @click="refresh">查询原请求</button>
  </section>
</template>
