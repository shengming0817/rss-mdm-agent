<script setup lang="ts">
import type { SelfServiceTask } from "./types";
defineProps<{ task: SelfServiceTask }>();
</script>
<template>
  <dl>
    <dt>运行身份</dt>
    <dd>{{ task.runAs === "system" ? "系统账户" : "本机登录用户" }}</dd>
    <template v-if="task.kind === 'software'">
      <dt>执行目标</dt>
      <dd v-if="task.target.kind === 'device'">本机设备</dd>
      <dd v-else>
        目标用户 {{ task.target.identity }} / 登录会话
        {{ task.target.sessionId }}
      </dd>
    </template>
  </dl>
</template>
