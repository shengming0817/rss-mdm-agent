<script setup lang="ts">
import type { BackendRequest } from "./types";
const props = defineProps<{ record: BackendRequest; disabled?: boolean }>();
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
  if (action.kind !== "policy") return {};
  return Object.fromEntries(
    Object.entries(action.directory.parameterSources).map(([key, source]) => [
      key,
      source.kind === "fixed" ? source.value : action.choice.parameters[key],
    ]),
  );
}
</script>
<template>
  <article class="policy-preparation" aria-label="原请求与确认">
    <template v-if="record.action.kind === 'policy'">
      <h3>{{ record.action.directory.selfService.displayName }}</h3>
      <p>{{ record.action.directory.selfService.description }}</p>
      <p>前置条件：{{ record.action.directory.selfService.prerequisites }}</p>
      <p>副作用：{{ record.action.directory.selfService.sideEffects }}</p>
      <dl>
        <dt>请求</dt>
        <dd>{{ request() }}</dd>
        <dt>策略 / 配置 revision / 执行版本</dt>
        <dd>
          {{ record.action.choice.policyId }} /
          {{ record.action.choice.policyRevision }} /
          {{ record.action.choice.policyVersion }}
        </dd>
        <dt>资源版本</dt>
        <dd>
          {{ record.action.directory.resource.id }} /
          {{ record.action.directory.resource.version }} /
          {{ record.action.directory.resource.variant }}
        </dd>
        <dt>企业账号</dt>
        <dd>
          {{ record.action.actor.tenantId }} /
          {{ record.action.actor.principalId }} · 登录
          {{ record.action.generation }}
        </dd>
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
      <button :disabled="disabled" @click="emit('query', request())">
        查询原请求与结果
      </button>
      <p>最终参数</p>
      <pre>{{ JSON.stringify(resolved(), null, 2) }}</pre>
      <p>风险：{{ record.risk?.level ?? "人工操作" }} · {{ record.state }}</p>
      <p v-if="record.state === 'awaitingConfirmation'">
        确认仅表示本人同意以上原请求；执行服务仍会核验当前策略与后台授权。
      </p>
      <button
        v-if="record.state === 'awaitingConfirmation'"
        :disabled="disabled"
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
