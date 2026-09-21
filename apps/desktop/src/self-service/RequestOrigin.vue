<script setup lang="ts">
import type { Plan } from "./types";
defineProps<{ plan: Pick<Plan, "actor" | "authority" | "initiator"> }>();
</script>
<template>
  <section class="request-origin" aria-label="冻结的请求来源">
    <h3>请求主体与来源</h3>
    <dl class="facts">
      <dt>请求主体</dt>
      <dd class="identifier">{{ plan.actor }}</dd>
      <dt>授权域</dt>
      <dd>
        {{ plan.authority.kind }} / {{ plan.authority.id
        }}<template v-if="plan.authority.kind === 'enterprise'">
          / 租户 {{ plan.authority.tenant }}</template
        >
      </dd>
      <dt>发起来源</dt>
      <dd>
        {{
          plan.initiator.kind === "human"
            ? "人工发起"
            : plan.initiator.kind === "ai"
              ? "AI 发起"
              : "策略发起"
        }}
      </dd>
      <template v-if="plan.initiator.kind !== 'policy'">
        <dt>来源设备</dt>
        <dd class="identifier">{{ plan.initiator.osSession.device }}</dd>
        <dt>来源系统账号</dt>
        <dd>
          {{ plan.initiator.osSession.account.platform }} /
          {{ plan.initiator.osSession.account.subject }}
        </dd>
        <dt>来源系统会话</dt>
        <dd class="identifier">{{ plan.initiator.osSession.session }}</dd>
      </template>
      <template v-if="plan.initiator.kind === 'ai'">
        <dt>AI 引擎</dt>
        <dd>{{ plan.initiator.provider }}</dd>
        <dt>AI 配置版本</dt>
        <dd>
          {{ plan.initiator.config.id }} /
          {{ plan.initiator.config.revision }}
        </dd>
        <dt>AI 会话</dt>
        <dd class="identifier">{{ plan.initiator.conversation }}</dd>
        <dt>工具调用</dt>
        <dd class="identifier">{{ plan.initiator.toolCall }}</dd>
      </template>
      <template v-if="plan.initiator.kind === 'policy'">
        <dt>来源策略</dt>
        <dd>
          {{ plan.initiator.policy.id }} / {{ plan.initiator.policy.revision }}
        </dd>
      </template>
    </dl>
    <p class="muted">
      来源随计划冻结；来源账号不授予执行权限，运行身份由计划单独指定。
    </p>
  </section>
</template>
