<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from "vue";
import { currentUser } from "../test-users";
import type { NetworkCheckPort } from "../self-service/native";
import type { NetworkCheckStatus, NetworkFinding } from "../self-service/types";
const props = withDefaults(
  defineProps<{
    port: NetworkCheckPort | null;
    active?: boolean;
    initialRequest?: string;
  }>(),
  { active: true },
);
const emit = defineEmits<{ task: [request: string] }>();
const host = ref(""),
  port = ref(443),
  request = ref(""),
  busy = ref(false),
  error = ref("");
const value = ref<NetworkCheckStatus>();
let epoch = 0,
  disposed = false;
const finished = computed(
  () =>
    !!value.value &&
    [
      "verified",
      "cancelled",
      "admissionDenied",
      "failedBeforeDispatch",
    ].includes(value.value.status.phase),
);
const phases: Record<string, string> = {
  waiting: "等待执行",
  admissionDenied: "不允许执行",
  approvalRequired: "等待批准",
  confirmationRequired: "等待确认",
  accepted: "已提交",
  running: "检查中",
  outcomeUnknown: "执行结果待核实",
  executionEnded: "执行结束，正在核实",
  verified: "检查已结束",
  failedBeforeDispatch: "启动失败",
  cancelled: "已取消",
};
const labels: Record<NetworkFinding, string> = {
  reachable: "可达",
  notRequired: "IP 地址，无需 DNS",
  dnsFailed: "解析失败",
  refused: "连接被拒绝",
  unreachable: "无法连接",
  timedOut: "超时",
};
async function defaults() {
  if (!props.port) return;
  const call = epoch;
  try {
    const response = await props.port({ kind: "defaults" });
    if (
      !disposed &&
      call === epoch &&
      !host.value &&
      response.kind === "defaults" &&
      response.value
    ) {
      host.value = response.value.host;
      port.value = response.value.port;
    }
  } catch {
    if (!disposed && call === epoch)
      error.value = "默认地址不可用，请填写域名/IP 和端口。";
  }
}
async function call(kind: "run" | "status" | "cancel") {
  if (
    !props.port ||
    busy.value ||
    disposed ||
    (kind !== "run" && !request.value)
  )
    return;
  if (kind === "run") {
    if (request.value && !finished.value) return;
    if (
      !host.value.trim() ||
      /[\s/@?#%;\\]/.test(host.value.trim()) ||
      !Number.isInteger(Number(port.value)) ||
      Number(port.value) < 1 ||
      Number(port.value) > 65535
    ) {
      error.value = "请填写有效域名/IP 和 1–65535 的端口。";
      return;
    }
    request.value = crypto.randomUUID();
    value.value = undefined;
  }
  const original = request.value,
    generation = epoch;
  busy.value = true;
  error.value = "";
  try {
    const response = await props.port(
      kind === "run"
        ? {
            kind,
            request: original,
            target: { host: host.value.trim(), port: Number(port.value) },
          }
        : { kind, request: original },
    );
    if (disposed || generation !== epoch) return;
    if (response.kind !== "status") throw Error("response");
    request.value = response.value.status.operationRequestId;
    value.value = response.value;
  } catch (failure) {
    if (!disposed && generation === epoch) {
      const code =
        failure && typeof failure === "object" && "code" in failure
          ? failure.code
          : "";
      if (
        (kind === "run" && code === "network_invalid_input") ||
        (kind === "status" && code === "network_not_found")
      ) {
        request.value = "";
        error.value = "原请求未创建，请修正参数后重试。";
      } else error.value = "操作未确认；请查询原请求，避免重复检查。";
    }
  } finally {
    if (generation === epoch) busy.value = false;
  }
}
watch(
  () => props.initialRequest,
  (original) => {
    if (!original || original === request.value) return;
    epoch++;
    busy.value = false;
    value.value = undefined;
    request.value = original;
    void call("status");
  },
  { immediate: true },
);
watch(
  () => currentUser.value?.generation,
  () => {
    ++epoch;
    host.value = "";
    port.value = 443;
    request.value = "";
    value.value = undefined;
    busy.value = false;
    error.value = "";
    if (props.active) void defaults();
  },
);
watch(
  () => props.active,
  (active) => {
    if (active) {
      if (request.value) void call("status");
      else void defaults();
    }
  },
  { immediate: true },
);
const polling = setInterval(() => {
  if (
    props.active &&
    request.value &&
    !finished.value &&
    value.value?.status.phase !== "outcomeUnknown"
  )
    void call("status");
}, 1000);
onBeforeUnmount(() => {
  disposed = true;
  ++epoch;
  clearInterval(polling);
});
</script>
<template>
  <section aria-label="连接检查">
    <form @submit.prevent="call('run')">
      <label
        >域名或 IP<input
          v-model="host"
          required
          maxlength="253"
          :disabled="!!request && !finished"
      /></label>
      <label
        >端口<input
          v-model="port"
          required
          type="number"
          min="1"
          max="65535"
          :disabled="!!request && !finished"
      /></label>
      <p>检查 DNS 和 TCP 端口连通性，不修改网络配置。</p>
      <button :disabled="!props.port || busy || (!!request && !finished)">
        {{ finished ? "重新检查" : "开始检查" }}
      </button>
    </form>
    <p v-if="!props.port">当前环境未连接执行服务，无法运行检查。</p>
    <p v-if="error" role="alert">{{ error }}</p>
    <section v-if="request" aria-label="原检查请求">
      <p>
        {{ (value ? phases[value.status.phase] : undefined) ?? "提交结果待查询"
        }}{{
          value?.status.cancelRequested && !finished
            ? " · 已请求取消，正在核实终止"
            : ""
        }}
      </p>
      <button :disabled="busy" @click="call('status')">查询原请求</button>
      <button
        :disabled="busy || finished || value?.status.cancelRequested"
        @click="call('cancel')"
      >
        取消检查
      </button>
      <button @click="emit('task', request)">查看原任务</button>
      <p
        v-if="finished && !value?.result && value?.status.phase !== 'cancelled'"
        role="alert"
      >
        检查未取得完整结果，请查看原任务。
      </p>
      <template v-if="value?.result">
        <p>
          {{ value.result.target.host }}:{{ value.result.target.port }} ·
          {{ new Date(value.result.collectedAtUnixMs).toLocaleString() }}
        </p>
        <p>
          DNS：{{ labels[value.result.dns.finding] }} ·
          {{ value.result.dns.durationMs }} ms
        </p>
        <p v-for="(step, i) in value.result.tcp" :key="i">
          {{ step.address }}：{{ labels[step.finding] }} ·
          {{ step.durationMs }} ms
        </p>
        <p>端口可达不代表应用服务健康或原症状已恢复。</p>
      </template>
    </section>
  </section>
</template>
