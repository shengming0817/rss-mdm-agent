<script setup lang="ts">
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
defineProps<{
  runtime: RuntimeClient;
  sessionId: string;
  instanceId: string;
}>();
import { mountNode } from "./NodeView";
</script>
<template>
  <RuntimeSurface
    :runtime="runtime"
    :session-id="sessionId"
    :instance-id="instanceId"
    v-bind="$attrs"
    :mount="mountNode"
    ><template #fallback><slot name="fallback" /></template
  ></RuntimeSurface>
</template>
