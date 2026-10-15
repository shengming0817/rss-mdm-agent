<script setup lang="ts">
import {
  CollapsibleRoot,
  CollapsibleTrigger,
  CollapsibleContent,
} from "reka-ui";
defineProps<{ open: boolean; label: string; disabled?: boolean }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
</script>
<template>
  <CollapsibleRoot
    :open="open"
    :disabled="disabled"
    class="ui-collapsible"
    @update:open="emit('update:open', $event)"
    ><CollapsibleTrigger
      >{{ label }}
      <span aria-hidden="true">{{ open ? "⌄" : "›" }}</span></CollapsibleTrigger
    >
    <div :hidden="!open">
      <CollapsibleContent :force-mount="true"><slot /></CollapsibleContent></div
  ></CollapsibleRoot>
</template>
