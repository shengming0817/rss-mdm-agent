<script setup lang="ts">
import { Monitor, Activity, Terminal, Settings } from "@rss-mdm-agent/ui";
defineProps<{ kind: string; size: number }>();
</script>
<template>
  <Monitor
    v-if="['display', 'lock', 'about'].includes(kind)"
    :size="size"
    aria-hidden="true"
  />
  <Activity
    v-else-if="['network', 'sound', 'power'].includes(kind)"
    :size="size"
    aria-hidden="true"
  />
  <Terminal v-else-if="kind === 'storage'" :size="size" aria-hidden="true" />
  <Settings v-else :size="size" aria-hidden="true" />
</template>
