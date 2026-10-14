<script setup lang="ts">
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
defineProps<{
  runtime: RuntimeClient;
  sessionId: string;
  instanceId: string;
}>();
defineEmits(["error", "receipt"]);
import { mountNode } from "./NodeView";
</script>
<template>
  <RuntimeSurface
    :runtime="runtime"
    :session-id="sessionId"
    :instance-id="instanceId"
    @error="$emit('error', $event)"
    @receipt="$emit('receipt', $event)"
    :mount="mountNode"
    ><template #fallback><slot name="fallback" /></template
  ></RuntimeSurface>
</template>
