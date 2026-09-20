<script setup lang="ts">
import RequestOrigin from "./RequestOrigin.vue";
import type { Plan } from "./types";
defineProps<{ plan: Plan }>();
</script>
<template>
  <section class="plan-summary" aria-label="确定性计划摘要">
    <div class="section-heading">
      <h2>计划摘要</h2>
      <span class="badge">固定测试计划</span>
    </div>
    <RequestOrigin :plan="plan" />
    <dl class="facts">
      <dt>计划 ID</dt>
      <dd class="identifier">{{ plan.planId }}</dd>
      <dt>请求 ID</dt>
      <dd class="identifier">{{ plan.requestId }}</dd>
      <dt>操作</dt>
      <dd>{{ plan.action }}</dd>
      <dt>精确资源</dt>
      <dd>
        {{ plan.resource.reference.id }} /
        {{ plan.resource.reference.revision }}
      </dd>
      <dt>目标</dt>
      <dd>{{ plan.target }}</dd>
      <dt>运行身份</dt>
      <dd>{{ plan.runAs }}</dd>
      <dt>权限</dt>
      <dd>{{ plan.permission }}</dd>
      <dt>网络范围</dt>
      <dd>{{ plan.network }}</dd>
      <dt>数据范围</dt>
      <dd>{{ plan.dataScope }}</dd>
      <dt>有效期至</dt>
      <dd>{{ new Date(plan.expiresAtUnixMs).toLocaleString() }}</dd>
    </dl>
    <ul class="parameter-summary">
      <li v-for="parameter in plan.parameters" :key="parameter.label">
        {{ parameter.label }}：{{ parameter.state }}
      </li>
    </ul>
    <details>
      <summary>计划内容摘要</summary>
      <p class="identifier">{{ plan.digest }}</p>
    </details>
  </section>
</template>
