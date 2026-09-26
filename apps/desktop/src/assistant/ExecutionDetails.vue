<script setup lang="ts">
import { computed } from "vue";
import RequestOrigin from "../self-service/RequestOrigin.vue";
import type {
  ExecutionTaskDetails,
  TaskPhase,
  ProcessEnd,
  ProcessFailureKind,
  OutputQuality,
  StopOutcome,
  EffectAssessment,
  SoftwareDiagnostic,
  DispatchCause,
  LimitReason,
} from "./execution-types";
const props = defineProps<{ details: ExecutionTaskDetails; now: number }>();
const validity = computed(() =>
  props.now < props.details.action.validity.notBeforeUnixMs
    ? "动作尚未生效"
    : props.now >= props.details.action.validity.expiresAtUnixMs
      ? "动作已过期"
      : "动作在有效期内",
);
const instant = (ms: number) =>
  ms >= -8_640_000_000_000_000 && ms <= 8_640_000_000_000_000
    ? new Date(ms).toISOString()
    : `${ms} Unix ms（超出本机日期格式范围）`;
function phase(value: TaskPhase): string {
  switch (value) {
    case "waiting":
      return "等待执行条件";
    case "admissionDenied":
      return "执行准入被拒绝";
    case "confirmationRequired":
      return "等待用户确认本次动作";
    case "approvalRequired":
      return "执行服务记录：需要管理员批准";
    case "accepted":
      return "执行意图已记录，尚未确认派发";
    case "running":
      return "执行器已接收，设备效果尚未确认";
    case "outcomeUnknown":
      return "设备效果未知，需要可信核对";
    case "executionEnded":
      return "执行已结束，等待效果验证";
    case "verified":
      return "执行结果已核实";
    case "failedBeforeDispatch":
      return "可信证据确认派发前失败";
    case "cancelled":
      return "可信证据确认取消且无效果";
  }
}
function ends(value: ProcessEnd): string {
  switch (value) {
    case "rejected":
      return "派发前已拒绝";
    case "exited":
      return "根进程退出（不代表效果成功）";
    case "cancelled":
      return "已请求取消";
    case "timedOut":
      return "执行时间额度已耗尽";
    case "outputLimit":
      return "累计输出额度已耗尽";
    case "unknown":
      return "结束原因未确认";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function qualities(value: OutputQuality): string {
  switch (value) {
    case "complete":
      return "完整且符合输出契约";
    case "truncated":
      return "输出已截断";
    case "failed":
      return "输出或进程结果不符合契约";
    case "partial":
      return "采集尚不完整";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function failures(value: ProcessFailureKind): string {
  switch (value) {
    case "none":
      return "未观察到机制故障";
    case "denied":
      return "权限或制品校验拒绝";
    case "unbound":
      return "身份或受控输入未绑定";
    case "capability":
      return "无法强制所需约束";
    case "unsupported":
      return "平台不支持所需调用";
    case "invalidInput":
      return "输入或配置格式无效";
    case "capacity":
      return "执行资源额度不足";
    case "conflict":
      return "执行关联发生冲突";
    case "unavailable":
      return "平台资源不可用";
    case "runtime":
      return "执行宿主初始化失败";
    case "spawn":
      return "目标进程启动失败";
    case "inputDelivery":
      return "受控输入未完整投递";
    case "capture":
      return "输出读取失败";
    case "supervision":
      return "进程状态无法可靠核实";
    case "outputValidation":
      return "输出编码或结构校验失败";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function stops(value: StopOutcome): string {
  switch (value) {
    case "acknowledged":
      return "停止请求已接收（未确认终止）";
    case "failed":
      return "停止请求未确认";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function softwareDiagnostic(value: SoftwareDiagnostic): string {
  switch (value) {
    case "cleanupPending":
      return "临时安装文件尚待安全清理，资源占用保留";
    case "cleanupUnverified":
      return "无法确认临时目录归属，需要人工核实";
    case "awaitingDetection":
      return "等待独立软件检测";
    case "restartPending":
      return "安装器要求重启设备；重启后重新核实";
    case "detectionUnavailable":
      return "软件检测不可用，请核对设备与读取权限";
    case "unrecognizedVersion":
      return "检测到未知软件内容，需要人工核实";
    case "detectionBudgetExceeded":
      return "软件检测预算耗尽，等待下一次有界核实";
    case "desiredStateObserved":
      return "已观察到目标软件状态；不代表后台活动已终止";
    case "desiredStateMissing":
      return "已检测软件状态，尚未达到目标";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function assessments(value: EffectAssessment): string {
  switch (value) {
    case "noEffect":
      return "已核实未产生效果";
    case "satisfied":
      return "已核实效果符合预期";
    case "notSatisfied":
      return "已核实效果不符合预期";
    case "unknown":
      return "效果未知，需可信核对";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function limits(value: LimitReason): string {
  switch (value) {
    case "notYetValid":
      return "尚未生效";
    case "expired":
      return "已过期";
    case "timeout":
      return "累计时间额度耗尽";
    case "output":
      return "累计输出额度耗尽";
    case "attempts":
      return "尝试次数已用尽";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function causes(value: Exclude<DispatchCause, object>): string {
  switch (value) {
    case "capabilityUnavailable":
      return "能力不可用";
    case "clockUnavailable":
      return "可信时间不可用";
    case "cancelled":
      return "请求已取消";
    case "staleRevision":
      return "可信快照已过期";
    case "runnerMismatch":
      return "执行器不匹配";
    case "configurationUnavailable":
      return "执行配置不可用";
    case "authorityUnavailable":
      return "执行授权不可用";
    case "lifecycleChanged":
      return "执行阶段已变化";
    case "runnerRejected":
      return "执行器拒绝派发";
    case "deliveryUnknown":
      return "派发结果未知，请核对原请求";
    case "runnerError":
      return "执行器发生错误";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function cause(value: DispatchCause | null): string {
  return value === null
    ? "无"
    : typeof value === "object"
      ? limits(value.limit)
      : causes(value);
}
type SoftwareAction = Extract<
  ExecutionTaskDetails["action"]["execution"],
  { kind: "software" }
>;
function adapterLabel(value: SoftwareAction["adapter"]): string {
  switch (value) {
    case "msi":
      return "Windows MSI";
    case "winget":
      return "WinGet";
    case "pkg":
      return "macOS PKG";
    case "homebrew":
      return "Homebrew";
    case "windowsBundle":
      return "Windows ZIP Bundle";
    case "macosBundle":
      return "macOS ZIP Bundle";
  }
  const exhaustive: never = value;
  return exhaustive;
}
function mutationLabel(value: SoftwareAction["mutation"]): string {
  switch (value) {
    case "install":
      return "安装";
    case "upgrade":
      return "升级";
    case "downgrade":
      return "降级";
    case "uninstall":
      return "卸载";
  }
  const exhaustive: never = value;
  return exhaustive;
}
const text = (value: unknown) => JSON.stringify(value, null, 2);
</script>
<template>
  <section class="execution-details" aria-label="可信执行详情">
    <h3>
      执行服务 ·
      {{ details.status.mode === "test" ? "S1 测试执行器" : "真实执行器" }}
    </h3>
    <p class="execution-phase">{{ phase(details.status.phase) }}</p>
    <p v-if="details.status.software" class="software-diagnostic">
      {{ softwareDiagnostic(details.status.software) }}
    </p>
    <div v-if="details.status.process" class="process-facts">
      <p>
        根进程：{{
          details.status.process.finished ? "已结束采集" : "运行或准备中"
        }}；退出码：{{ details.status.process.exitCode ?? "未知" }}
      </p>
      <p>
        执行范围静止：{{
          details.status.process.quiescent ? "已确认" : "未确认"
        }}；输出质量：{{ qualities(details.status.process.quality) }}
      </p>
      <p>结束原因：{{ ends(details.status.process.end) }}</p>
      <p>机制诊断：{{ failures(details.status.process.failureKind) }}</p>
      <p v-if="details.status.process.failureKind !== 'none'">
        请核对原请求与宿主诊断；不要因未收到完整结果而重复派发。
      </p>
    </div>
    <p class="plan-validity">
      {{ validity }}（按本机时间判断；实际准入由执行服务核验）。
    </p>
    <p v-if="validity !== '动作在有效期内'">
      这是已读取的冻结动作与历史阶段；请重新读取详情，必要时重新发起新请求。不要据此重复派发。
    </p>
    <p v-if="details.status.mode === 'test'">
      测试结果不代表真实设备变更或生产接线完成。
    </p>
    <p v-if="details.status.cancelRequested">
      执行取消已请求；取消意图、停止响应和效果验证分别记录。
    </p>
    <RequestOrigin :input="details.action" />
    <dl>
      <dt>原始执行请求</dt>
      <dd>{{ details.status.operationRequestId }}</dd>
      <dt>冻结动作 / 摘要</dt>
      <dd>
        {{ details.action.requestId }}<br />{{ details.action.contentDigest }}
      </dd>
      <template v-if="details.action.execution.kind === 'software'">
        <dt>软件执行</dt>
        <dd class="software-operation">
          {{ adapterLabel(details.action.execution.adapter) }}
          ·
          {{ mutationLabel(details.action.execution.mutation) }}
        </dd>
      </template>
      <dt>当前尝试</dt>
      <dd>
        {{ details.status.attemptId ?? "尚未准入" }} · 共
        {{ details.status.attempts }} 次
      </dd>
      <dt>目标</dt>
      <dd>
        <pre>{{ text(details.action.target) }}</pre>
      </dd>
      <dt>运行身份</dt>
      <dd>
        <pre>{{ text(details.action.runAs) }}</pre>
      </dd>
      <dt>操作与资源版本</dt>
      <dd>
        <pre>{{ text(details.action.operation) }}</pre>
      </dd>
      <dt>精确制品</dt>
      <dd>
        <pre>{{ text(details.action.artifact) }}</pre>
      </dd>
      <dt>解释器</dt>
      <dd>
        <pre>{{ text(details.action.interpreter) }}</pre>
      </dd>
      <dt>策略版本</dt>
      <dd>
        <pre>{{ text(details.action.policy) }}</pre>
      </dd>
      <dt>用户会话要求</dt>
      <dd>
        <pre>{{ text(details.action.sessionRequirement) }}</pre>
      </dd>
      <dt>有效期</dt>
      <dd>
        生效：{{ instant(details.action.validity.notBeforeUnixMs) }}<br />
        到期（不含）：{{ instant(details.action.validity.expiresAtUnixMs) }}
      </dd>
      <dt>累计预算</dt>
      <dd>
        <pre>{{ text(details.action.budget) }}</pre>
      </dd>
      <dt>访问范围</dt>
      <dd>
        <pre>{{ text(details.action.access) }}</pre>
      </dd>
      <dt>派发诊断 / 停止响应</dt>
      <dd>
        {{ cause(details.status.dispatchCause) }} /
        {{
          details.status.stopOutcome === null
            ? "无"
            : stops(details.status.stopOutcome)
        }}
      </dd>
      <dt>效果验证</dt>
      <dd>
        {{
          details.status.assessment === null
            ? "尚未验证"
            : assessments(details.status.assessment)
        }}
      </dd>
      <dt>终止/效果核验证据引用（仅授权可见）</dt>
      <dd>
        <pre>{{ text(details.status.evidence) }}</pre>
      </dd>
    </dl>
  </section>
</template>
