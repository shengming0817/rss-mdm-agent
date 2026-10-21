<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { currentUser } from "../test-users";
import { nativePort } from "../self-service/native";
import TaskDetail from "../self-service/TaskDetail.vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import type {
  BackendRequest,
  BackendTask,
  ExecutionTaskDetails,
  SelfServicePort,
} from "../self-service/types";
import type { MdmStatus } from "./service-contract";
import { readMdmStatus, type ServicePort } from "./native";
const props = withDefaults(
  defineProps<{
    servicePort?: Pick<ServicePort, "mdmStatus">;
    executionPort?: SelfServicePort;
    fixture?: boolean;
    agentReady?: boolean;
    active?: boolean;
  }>(),
  { agentReady: true, active: true },
);
const status = ref<MdmStatus>(),
  error = ref(""),
  busy = ref(false),
  pending = ref(true);
const records = ref<BackendRequest[]>([]),
  available = ref<BackendTask[]>([]),
  executions = ref<ExecutionTaskDetails[]>([]);
function executionOwner() {
  if (props.fixture) return props.executionPort ?? null;
  return (
    props.executionPort ??
    (props.agentReady && currentUser.value ? nativePort() : null)
  );
}
const port = ref(executionOwner());
let timer: ReturnType<typeof setInterval> | undefined;
let epoch = 0;
let disposed = false;
function clearTasks() {
  records.value = [];
  executions.value = [];
  available.value = [];
  pending.value = true;
}
const text = computed(() => {
  switch (status.value?.state) {
    case "unenrolled":
      return "本机尚未注册 MDM。";
    case "this_organization":
      return "本机检测到本组织的 MDM 注册。";
    case "other_organization":
      return "本机已由其他组织管理，不会自动替换。";
    default:
      return "无法确定本机 MDM 注册状态。";
  }
});
async function refresh() {
  if (busy.value || !props.active) return;
  busy.value = true;
  const version = epoch;
  const owner = port.value;
  const current = () => !disposed && props.active && version === epoch;
  try {
    const nextStatus: MdmStatus = props.servicePort
      ? await props.servicePort.mdmStatus()
      : props.fixture
        ? { state: "unknown" }
        : await readMdmStatus();
    if (!current()) return;
    if (owner && props.agentReady) {
      const snapshot = await owner.snapshot({
        after: null,
        selected: null,
      });
      if (!current()) return;
      const nextExecutions = snapshot.requests.filter(
        (t) => t.action.operation.action === "backend-enrollment",
      );
      const statuses = await Promise.all(
        snapshot.preparations
          .filter(
            (r) =>
              r.action.kind === "policy" &&
              r.action.directory.task.kind === "enrollment" &&
              !["failed", "cancelled"].includes(r.state),
          )
          .map(async (r) => {
            if (r.action.kind !== "policy") return false;
            if (r.state !== "observed") return true;
            const result = await owner.policy_status({
              requestId: r.action.choice.request,
            });
            if (result.kind === "absent") return false;
            if (result.kind !== "run")
              return !["failed", "cancelled"].includes(result.value.state);
            return (
              result.value.deadline > Date.now() / 1000 &&
              result.value.state.cancellation !== "confirmed" &&
              result.value.state.execution !== "failed" &&
              (result.value.state.execution !== "succeeded" ||
                (result.value.result?.kind === "enrollment" &&
                  result.value.result.outcome === "opened"))
            );
          }),
      );
      if (!current()) return;
      records.value = snapshot.preparations;
      executions.value = nextExecutions;
      pending.value =
        statuses.some((value) => value) ||
        nextExecutions.some(
          (t) =>
            ![
              "verified",
              "executionEnded",
              "failedBeforeDispatch",
              "cancelled",
              "admissionDenied",
            ].includes(t.status.phase),
        ) ||
        snapshot.available.some(
          (t) => t.presentation.summary.kind === "enrollment",
        );
      available.value = snapshot.available.filter(
        (t) => t.presentation.summary.kind === "enrollment",
      );
    } else {
      clearTasks();
    }
    status.value = nextStatus;
    error.value = "";
  } catch {
    if (!current()) return;
    error.value = "无法读取本机服务，请刷新重试。";
    status.value = undefined;
    clearTasks();
  } finally {
    busy.value = false;
    if (!disposed && props.active && version !== epoch) void refresh();
  }
}
// ref: Vue v3.5.38 apiWatch.ts — invalidate async work when its owner changes.
watch(
  [
    () => currentUser.value?.generation,
    () => props.agentReady,
    () => props.active,
    () => props.executionPort,
    () => props.servicePort,
    () => props.fixture,
  ],
  () => {
    epoch++;
    port.value = executionOwner();
    status.value = undefined;
    error.value = "";
    clearTasks();
    if (props.active) void refresh();
  },
  { flush: "sync" },
);
onMounted(() => {
  void refresh();
  timer = setInterval(refresh, 5000);
});
onUnmounted(() => {
  disposed = true;
  epoch++;
  clearInterval(timer);
});
</script>
<template>
  <section class="mdm-registration" aria-label="MDM 注册">
    <header>
      <div>
        <h3>MDM 注册</h3>
        <p>系统 MDM 与 Agent 设备身份分别核实。</p>
      </div>
      <span
        class="mdm-status"
        :class="{ enrolled: status?.state === 'this_organization' }"
        >{{
          status?.state === "this_organization"
            ? "本组织管理"
            : status?.state === "other_organization"
              ? "其他组织管理"
              : status?.state === "unenrolled"
                ? "未注册"
                : "待核实"
        }}</span
      >
    </header>
    <p>{{ text }}</p>
    <p v-if="error" role="status">{{ error }}</p>
    <button :disabled="busy" @click="refresh">刷新注册状态</button>
    <p v-if="available.length">
      组织已下发 MDM 注册任务，等待当前登录会话完成系统操作。
    </p>
    <p v-if="!agentReady">
      完成设备注册并恢复有效设备凭据后，可继续组织授权的 MDM 注册任务。
    </p>
    <p v-if="status?.observedAt" class="mdm-observed">
      最近核实：{{ new Date(status.observedAt * 1000).toLocaleString() }}
    </p>
    <p v-if="pending && !error && port && agentReady">
      已有注册流程，请查询或继续原请求。
    </p>
    <TaskDetail
      v-for="task in executions"
      :key="task.action.requestId"
      :task="task"
      :disabled="busy"
      @cancel="port?.cancel({ requestId: task.action.requestId }).then(refresh)"
    />
    <PolicySelfService
      v-if="status && agentReady && port"
      :port="port"
      :records="records"
      kind="enrollment"
      :allow-new-requests="status.state === 'unenrolled' && !pending"
      :allow-confirm="status.state === 'unenrolled'"
      @changed="refresh"
    />
  </section>
</template>

<style scoped>
.mdm-registration {
  min-width: 0;
  padding: 22px 24px;
  margin-top: 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  overflow-wrap: anywhere;
}
header {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}
h3 {
  font-size: 14px;
  font-weight: 600;
  margin: 0 0 6px;
}
p {
  font-size: 12px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
  margin: 10px 0;
}
header p {
  font-size: 11px;
  margin: 0;
}
.mdm-status {
  flex: none;
  padding: 4px 9px;
  border-radius: 999px;
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text-muted);
  font-size: 10px;
}
.mdm-status.enrolled {
  color: var(--rss-color-success);
  background: var(--rss-color-success-bg);
}
.mdm-observed {
  font-size: 10px;
}
button {
  min-height: 34px;
  padding: 7px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  font-size: 12px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
}
.mdm-registration :deep(.policy-self-service > .section-heading h3) {
  display: none;
}
.mdm-registration :deep(.policy-self-service) {
  border: 0;
  padding: 12px 0 0;
}
@media (max-width: 700px) {
  .mdm-registration {
    padding: 18px 16px;
  }
}
</style>
