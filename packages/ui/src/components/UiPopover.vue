<script setup lang="ts">
import {
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
} from "reka-ui";
import { useUiContext } from "../context";
defineProps<{ open: boolean; label: string }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { portal } = useUiContext();
</script>
<template>
  <PopoverRoot :open="open" @update:open="emit('update:open', $event)"
    ><PopoverTrigger as-child><slot name="trigger" /></PopoverTrigger
    ><PopoverPortal v-if="portal"
      ><PopoverContent
        class="ui-popup ui-popover"
        :aria-label="label"
        :side-offset="6"
        ><slot /></PopoverContent></PopoverPortal
  ></PopoverRoot>
</template>
