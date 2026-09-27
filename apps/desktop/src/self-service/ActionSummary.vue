<script setup lang="ts">
import RequestOrigin from "./RequestOrigin.vue";
import type { Action } from "./types";
defineProps<{ input: Action }>();
function riskLabel(level: number | null): string {
  switch (level) {
    case 0:
      return "0 · 纯计算，无外部副作用；AI 可在权限内直接执行";
    case 1:
      return "1 · 有界非敏感只读；AI 可在权限内直接执行";
    case 2:
      return "2 · 策略允许的受限副作用；AI 需要用户确认";
    case 3:
      return "3 · 破坏性或安全敏感操作；AI 默认阻止";
    default:
      return "未知 · 无可信分类；AI 默认阻止";
  }
}
</script>
<template>
  <section class="action-summary" aria-label="确定性动作摘要">
    <div class="section-heading">
      <h2>动作摘要</h2>
      <span class="badge">固定测试动作</span>
    </div>
    <RequestOrigin :input="input" />
    <p>风险等级不授予权限；人工动作仍需本人确认。</p>
    <dl class="facts">
      <dt>请求 ID</dt>
      <dd class="identifier">{{ input.requestId }}</dd>
      <dt>风险等级</dt>
      <dd>{{ riskLabel(input.riskLevel) }}</dd>
      <dt>操作</dt>
      <dd>{{ input.action }}</dd>
      <dt>精确资源</dt>
      <dd>
        {{ input.resource.reference.id }} /
        {{ input.resource.reference.revision }}
      </dd>
      <dt>目标</dt>
      <dd>{{ input.target }}</dd>
      <dt>运行身份</dt>
      <dd>{{ input.runAs }}</dd>
      <dt>权限</dt>
      <dd>{{ input.permission }}</dd>
      <dt>网络范围</dt>
      <dd>{{ input.network }}</dd>
      <dt>数据范围</dt>
      <dd>{{ input.dataScope }}</dd>
      <dt>有效期至</dt>
      <dd>{{ new Date(input.expiresAtUnixMs).toLocaleString() }}</dd>
    </dl>
    <ul class="parameter-summary">
      <li v-for="parameter in input.parameters" :key="parameter.label">
        {{ parameter.label }}：{{ parameter.value ?? parameter.state }}
      </li>
    </ul>
    <details>
      <summary>动作内容摘要</summary>
      <p class="identifier">{{ input.digest }}</p>
    </details>
  </section>
</template>
