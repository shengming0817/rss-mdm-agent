<script setup lang="ts">
import type { BackendRequest } from "./types";
import PolicyExecutionContext from "./PolicyExecutionContext.vue";
const props = defineProps<{
  record: BackendRequest;
  disabled?: boolean;
  confirmDisabled?: boolean;
}>();
const emit = defineEmits<{
  confirm: [request: string];
  cancel: [request: string];
  query: [request: string];
}>();
function request() {
  const action = props.record.action;
  return action.kind === "offered"
    ? action.offer.request
    : action.choice.request;
}
function resolved() {
  const action = props.record.action;
  if (action.kind !== "policy" || action.task.kind !== "script") return {};
  return Object.fromEntries(
    Object.entries(action.task.parameterSources).map(([key, source]) => [
      key,
      source.kind === "fixed" ? source.value : action.choice.parameters[key],
    ]),
  );
}
function presentationText(value: unknown, fallback = "未提供"): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}
</script>
<template>
  <article class="policy-preparation" aria-label="原请求与确认">
    <template v-if="record.action.kind === 'policy'">
      <h3>
        {{
          presentationText(
            record.action.presentation.displayName,
            `策略 ${record.action.choice.policyId}`,
          )
        }}
      </h3>
      <p>
        {{ presentationText(record.action.presentation.description) }}
      </p>
      <p>
        前置条件：{{
          presentationText(record.action.presentation.prerequisites)
        }}
      </p>
      <p>
        副作用：{{ presentationText(record.action.presentation.sideEffects) }}
      </p>
      <dl>
        <dt>请求</dt>
        <dd>{{ request() }}</dd>
        <dt>策略</dt>
        <dd>{{ record.action.choice.policyId }}</dd>
        <dt>资源版本</dt>
        <dd v-if="record.action.task.kind !== 'enrollment'">
          {{ record.action.task.resource.id }} /
          {{ record.action.task.resource.version }} /
          {{ record.action.task.resource.variant }}
        </dd>
        <dd v-else>原生 MDM 注册 · {{ record.action.task.organization }}</dd>
        <dt>准入主体</dt>
        <dd>
          {{
            record.action.access.kind === "device"
              ? "设备准入 · 无需企业登录"
              : "企业用户准入"
          }}
        </dd>
        <template v-if="record.action.access.kind === 'user'">
          <dt>发起企业身份 / 会话</dt>
          <dd>
            {{ record.action.access.sessionBinding.generation }}
          </dd>
        </template>
        <template v-if="record.trigger.kind !== 'automatic'"
          ><dt>发起设备 / OS 账号 / 登录</dt>
          <dd>
            {{ record.trigger.osSession.device }} /
            {{ record.trigger.osSession.account.subject }} /
            {{ record.trigger.osSession.session }}
          </dd></template
        >
        <template v-if="record.trigger.kind === 'ai'"
          ><dt>AI 会话 / 调用</dt>
          <dd>
            {{ record.trigger.conversation }} / {{ record.trigger.toolCall }}
          </dd></template
        >
      </dl>
      <PolicyExecutionContext :task="record.action.task" />
      <button :disabled="disabled" @click="emit('query', request())">
        查询原请求与结果
      </button>
      <template v-if="record.action.task.kind === 'script'">
        <p>最终参数</p>
        <pre>{{ JSON.stringify(resolved(), null, 2) }}</pre>
      </template>
      <p>风险：{{ record.risk?.level ?? "人工操作" }} · {{ record.state }}</p>
      <p v-if="record.state === 'awaitingConfirmation'">
        确认仅表示本人同意以上原请求；执行服务仍会核验当前策略与后台授权。
      </p>
      <button
        v-if="record.state === 'awaitingConfirmation'"
        :disabled="disabled || confirmDisabled"
        @click="emit('confirm', request())"
      >
        确认上述操作
      </button>
      <button
        v-if="!['failed', 'cancelled', 'observed'].includes(record.state)"
        :disabled="disabled"
        @click="emit('cancel', request())"
      >
        撤销原请求
      </button>
      <p v-if="record.failure === 'expired'">
        本地授权已到期，停止提交。仍可查询原请求的后台结果。
      </p>
      <p v-else-if="record.failure">{{ record.failure }}</p>
    </template>
    <template v-else-if="record.action.kind === 'softwareRead'">
      <h3>软件状态检测</h3>
      <p>规则：{{ record.action.definition }} · 有限原生软件查询</p>
      <p>请求：{{ request() }} · {{ record.state }}</p>
      <template v-if="record.action.observation"
        ><p>
          {{ record.action.observation.package }} ·
          {{ record.action.observation.quality }} · 目标版本
          {{ record.action.observation.expectedVersion }}
        </p>
        <p>
          采集：{{
            new Date(
              record.action.observation.collectedAtUnixMs,
            ).toLocaleString()
          }}
        </p>
        <pre>{{
          JSON.stringify(record.action.observation.state, null, 2)
        }}</pre>
      </template>
    </template>
  </article>
</template>
