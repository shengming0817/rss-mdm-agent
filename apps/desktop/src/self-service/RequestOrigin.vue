<script setup lang="ts">
import type { Action } from "./types";
defineProps<{ input: Pick<Action, "actor" | "authority" | "initiator"> }>();
</script>
<template>
  <section class="request-origin" aria-label="冻结的请求来源">
    <h3>请求主体与来源</h3>
    <dl class="facts">
      <dt>请求主体</dt>
      <dd class="identifier">{{ input.actor }}</dd>
      <dt>授权域</dt>
      <dd>
        {{ input.authority.kind }} / {{ input.authority.id
        }}<template v-if="input.authority.kind === 'enterprise'">
          / 租户 {{ input.authority.tenant }}</template
        >
      </dd>
      <dt>发起来源</dt>
      <dd>
        {{
          input.initiator.kind === "human"
            ? "人工发起"
            : input.initiator.kind === "ai"
              ? "AI 发起"
              : "策略发起"
        }}
      </dd>
      <template v-if="input.initiator.kind !== 'policy'">
        <dt>来源设备</dt>
        <dd class="identifier">{{ input.initiator.osSession.device }}</dd>
        <dt>来源系统账号</dt>
        <dd>
          {{ input.initiator.osSession.account.platform }} /
          {{ input.initiator.osSession.account.subject }}
        </dd>
        <dt>来源系统会话</dt>
        <dd class="identifier">{{ input.initiator.osSession.session }}</dd>
      </template>
      <template v-if="input.initiator.kind === 'ai'">
        <dt>AI 引擎</dt>
        <dd>{{ input.initiator.provider }}</dd>
        <dt>AI 配置版本</dt>
        <dd>
          {{ input.initiator.config.id }} /
          {{ input.initiator.config.revision }}
        </dd>
        <dt>AI 会话</dt>
        <dd class="identifier">{{ input.initiator.conversation }}</dd>
        <dt>工具调用</dt>
        <dd class="identifier">{{ input.initiator.toolCall }}</dd>
      </template>
      <template v-if="input.initiator.kind === 'policy'">
        <dt>来源策略</dt>
        <dd>
          {{ input.initiator.policy.id }} /
          {{ input.initiator.policy.revision }}
        </dd>
      </template>
    </dl>
    <p class="muted">
      来源随动作冻结；来源账号不授予执行权限，运行身份由动作单独指定。
    </p>
  </section>
</template>
