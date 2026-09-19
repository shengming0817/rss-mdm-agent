<script setup lang="ts">
import { computed } from "vue";
import type { ExecutionTaskDetails, TaskPhase } from "./execution-types";
const props = defineProps<{ details: ExecutionTaskDetails; now: number }>();
const validity = computed(() =>
  props.now < props.details.plan.validity.notBeforeUnixMs
    ? "计划尚未生效"
    : props.now >= props.details.plan.validity.expiresAtUnixMs
      ? "计划已过期"
      : "计划在有效期内",
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
    case "testCompleted":
      return "S1 测试流程完成";
    case "failedBeforeDispatch":
      return "可信证据确认派发前失败";
    case "cancelled":
      return "可信证据确认取消且无效果";
  }
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
    <p class="plan-validity">
      {{ validity }}（按本机时间判断；实际准入由执行服务核验）。
    </p>
    <p v-if="validity !== '计划在有效期内'">
      这是已读取的冻结计划与历史阶段；请重新读取详情，必要时获取新冻结计划。不要据此重复派发。
    </p>
    <p v-if="details.status.mode === 'test'">
      测试结果不代表真实设备变更或生产接线完成。
    </p>
    <p v-if="details.status.cancelRequested">
      执行取消已请求；取消意图、停止响应和效果验证分别记录。
    </p>
    <dl>
      <dt>原始执行请求</dt>
      <dd>{{ details.status.operationRequestId }}</dd>
      <dt>冻结计划 / 摘要</dt>
      <dd>{{ details.plan.planId }}<br />{{ details.plan.planDigest }}</dd>
      <dt>当前尝试</dt>
      <dd>
        {{ details.status.attemptId ?? "尚未准入" }} · 共
        {{ details.status.attempts }} 次
      </dd>
      <dt>目标</dt>
      <dd>
        <pre>{{ text(details.plan.target) }}</pre>
      </dd>
      <dt>运行身份</dt>
      <dd>
        <pre>{{ text(details.plan.runAs) }}</pre>
      </dd>
      <dt>操作与资源版本</dt>
      <dd>
        <pre>{{ text(details.plan.operation) }}</pre>
      </dd>
      <dt>精确制品</dt>
      <dd>
        <pre>{{ text(details.plan.artifact) }}</pre>
      </dd>
      <dt>解释器</dt>
      <dd>
        <pre>{{ text(details.plan.interpreter) }}</pre>
      </dd>
      <dt>策略版本</dt>
      <dd>
        <pre>{{ text(details.plan.policy) }}</pre>
      </dd>
      <dt>用户会话要求</dt>
      <dd>
        <pre>{{ text(details.plan.sessionRequirement) }}</pre>
      </dd>
      <dt>有效期</dt>
      <dd>
        生效：{{ instant(details.plan.validity.notBeforeUnixMs) }}<br />
        到期（不含）：{{ instant(details.plan.validity.expiresAtUnixMs) }}
      </dd>
      <dt>累计预算</dt>
      <dd>
        <pre>{{ text(details.plan.budget) }}</pre>
      </dd>
      <dt>访问范围</dt>
      <dd>
        <pre>{{ text(details.plan.access) }}</pre>
      </dd>
      <dt>派发诊断 / 停止响应</dt>
      <dd>
        {{ text(details.status.dispatchCause) }} /
        {{ details.status.stopOutcome ?? "无" }}
      </dd>
      <dt>效果验证</dt>
      <dd>{{ details.status.assessment ?? "尚未验证" }}</dd>
      <dt>授权证据引用</dt>
      <dd>
        <pre>{{ text(details.status.evidence) }}</pre>
      </dd>
    </dl>
  </section>
</template>
