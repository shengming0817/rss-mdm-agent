<script setup lang="ts">
import RequestOrigin from "./RequestOrigin.vue";
import type { Action } from "./types";
defineProps<{ action: Action }>();
</script>
<template>
  <section class="action-summary" aria-label="确定性动作摘要">
    <div class="section-heading">
      <h2>动作摘要</h2>
      <span class="badge">固定测试动作</span>
    </div>
    <RequestOrigin :plan="action" />
    <dl class="facts">
      <dt>请求 ID</dt>
      <dd class="identifier">{{ action.requestId }}</dd>
      <dt>风险等级</dt>
      <dd>{{ action.riskLevel ?? "未知" }}</dd>
      <dt>操作</dt>
      <dd>{{ action.action }}</dd>
      <dt>精确资源</dt>
      <dd>
        {{ action.resource.reference.id }} /
        {{ action.resource.reference.revision }}
      </dd>
      <dt>目标</dt>
      <dd>{{ action.target }}</dd>
      <dt>运行身份</dt>
      <dd>{{ action.runAs }}</dd>
      <dt>权限</dt>
      <dd>{{ action.permission }}</dd>
      <dt>网络范围</dt>
      <dd>{{ action.network }}</dd>
      <dt>数据范围</dt>
      <dd>{{ action.dataScope }}</dd>
      <dt>有效期至</dt>
      <dd>{{ new Date(action.expiresAtUnixMs).toLocaleString() }}</dd>
    </dl>
    <ul class="parameter-summary">
      <li v-for="parameter in action.parameters" :key="parameter.label">
        {{ parameter.label }}：{{ parameter.value ?? parameter.state }}
      </li>
    </ul>
    <details>
      <summary>动作内容摘要</summary>
      <p class="identifier">{{ action.digest }}</p>
    </details>
  </section>
</template>
