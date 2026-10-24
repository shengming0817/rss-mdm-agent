<script setup lang="ts">
import { computed } from "vue";
import type { Controller } from "./controller";
import TaskDetail from "./TaskDetail.vue";
import PolicySelfService from "./PolicySelfService.vue";
import PolicyPreparation from "./PolicyPreparation.vue";
const props = defineProps<{ controller: Controller }>();
const c = props.controller,
  s = c.state;
const task = computed(() =>
  s.snapshot?.selected?.kind === "execution"
    ? s.snapshot.selected.value
    : s.snapshot?.requests.find((t) => t.action.requestId === s.taskId),
);
function operationLabel(value: string) {
  switch (value) {
    case "install":
      return "安装";
    case "uninstall":
      return "卸载";
    case "detect":
      return "检测";
    default:
      return "未知操作";
  }
}
function stateLabel(value: string) {
  switch (value) {
    case "awaitingConfirmation":
      return "等待本人确认";
    case "ready":
      return "待执行";
    case "submitting":
      return "准备执行";
    case "failed":
      return "准备失败；没有创建新尝试";
    case "cancelled":
      return "已撤销";
    default:
      return "状态未知";
  }
}
function failureLabel(value: string) {
  switch (value) {
    case "riskUnknown":
      return "后台未提供可信风险等级，AI 操作已拒绝";
    case "riskBlocked":
      return "后台风险策略禁止 AI 执行此操作";
    case "preparationFailed":
      return "本机条件或后台 Start 未满足";
    case "interrupted":
      return "服务中断，未重新执行";
    case "expired":
      return "原任务已过期";
    case "revoked":
      return "后台授权已撤销";
    default:
      return "状态未知";
  }
}
</script>
<template>
  <section class="task-records self-service" aria-label="请求与任务">
    <header class="catalog-heading">
      <h2>执行服务记录</h2>
      <button :disabled="s.loading" @click="c.refresh">刷新</button>
    </header>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
    <template v-if="s.snapshot"
      ><section class="preparation-list" aria-label="准备中的请求">
        <h3 v-if="s.snapshot.preparations.length">准备中的请求</h3>
        <article
          v-for="pending in s.snapshot.preparations.filter(
            (p) => p.action.kind === 'offered',
          )"
          :key="
            pending.action.kind === 'offered'
              ? pending.action.offer.request
              : ''
          "
          v-show="pending.action.kind === 'offered'"
        >
          <template v-if="pending.action.kind === 'offered'">
            <strong>{{ pending.action.offer.presentation.title }}</strong>
            <template
              v-if="
                pending.action.offer.presentation.summary.kind === 'software'
              "
            >
              <p>
                操作：{{
                  operationLabel(
                    pending.action.offer.presentation.summary.intent,
                  )
                }}
              </p>
              <ol>
                <li
                  v-for="(step, index) in pending.action.offer.presentation
                    .summary.steps"
                  :key="index"
                >
                  {{ step.package }} {{ step.version }} ·
                  {{ step.identity === "system" ? "系统账号" : "当前用户" }}
                </li>
              </ol>
            </template>
            <p v-else>
              {{
                pending.action.offer.presentation.summary.kind === "collection"
                  ? "原生设备采集"
                  : "后台固定脚本"
              }}
              ·
              {{
                pending.action.offer.presentation.summary.kind ===
                  "collection" ||
                (pending.action.offer.presentation.summary.kind === "script" &&
                  pending.action.offer.presentation.summary.identity ===
                    "system")
                  ? "系统账号"
                  : "当前用户"
              }}
            </p>
            <p v-if="pending.trigger.kind === 'ai'">
              AI 风险：{{ pending.risk?.level ?? "后台未提供" }}
            </p>
            <p>
              {{ stateLabel(pending.state)
              }}<span v-if="pending.failure">
                ·
                {{ failureLabel(pending.failure) }}</span
              >
            </p>
            <button
              v-if="pending.state === 'awaitingConfirmation'"
              :disabled="s.busy || s.uncertain"
              @click="c.confirmPreparation(pending.action.offer)"
            >
              确认此操作
            </button>
            <button
              v-if="!['failed', 'cancelled'].includes(pending.state)"
              :disabled="s.busy"
              @click="c.cancel(pending.action.offer.request)"
            >
              撤销请求
            </button>
          </template>
        </article>
      </section>
      <PolicyPreparation
        v-for="pending in s.snapshot.preparations.filter(
          (p) => p.action.kind === 'softwareRead',
        )"
        :key="
          pending.action.kind === 'offered'
            ? pending.action.offer.request
            : pending.action.choice.request
        "
        :record="pending"
      />
      <PolicySelfService
        :port="c.port"
        :records="s.snapshot.preparations"
        kind="all"
        :allow-new-requests="false"
        @changed="c.refresh"
      />
      <section class="execution-records" aria-label="执行记录">
        <h3 v-if="s.snapshot.requests.length">执行记录</h3>
        <button
          v-for="record in s.snapshot.requests"
          :key="record.action.requestId"
          @click="s.taskId = record.action.requestId"
        >
          {{ record.action.operation.resource.id }} · {{ record.status.phase }}
        </button>
        <button v-if="s.previous.length" :disabled="s.loading" @click="c.first">
          首页
        </button>
        <button
          v-if="s.previous.length"
          :disabled="s.loading"
          @click="c.previous"
        >
          上一页
        </button>
        <button v-if="s.snapshot.next" :disabled="s.loading" @click="c.next">
          下一页
        </button>
      </section>
      <TaskDetail
        v-if="task"
        :task="task"
        :disabled="s.busy"
        @cancel="c.cancel(task.action.requestId)"
      />
    </template>
  </section>
</template>
