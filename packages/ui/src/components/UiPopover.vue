<script setup lang="ts">
import { useId } from "vue";
import {
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
} from "reka-ui";
import { useUiContext } from "../context";
defineProps<{ open: boolean; label: string; contentClass?: string }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { portal } = useUiContext();
const titleId = `ui-popover-title-${useId()}`;
</script>
<template>
  <PopoverRoot :open="open" @update:open="emit('update:open', $event)"
    ><PopoverTrigger as-child><slot name="trigger" /></PopoverTrigger
    ><PopoverPortal v-if="portal"
      ><PopoverContent
        class="ui-popup ui-popover"
        :class="contentClass"
        :aria-label="label"
        :aria-labelledby="titleId"
        :side-offset="6"
        ><span :id="titleId" class="sr-only">{{ label }}</span
        ><slot /></PopoverContent></PopoverPortal
  ></PopoverRoot>
</template>
