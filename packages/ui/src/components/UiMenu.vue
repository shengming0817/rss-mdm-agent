<script setup lang="ts">
import {
  DropdownMenuRoot,
  DropdownMenuTrigger,
  DropdownMenuPortal,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuLabel,
} from "reka-ui";
import { useUiContext, type UiItem } from "../context";
defineProps<{
  open: boolean;
  label: string;
  items: readonly (UiItem & { group?: string })[];
}>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  select: [id: string];
}>();
const { portal } = useUiContext();
</script>
<template>
  <DropdownMenuRoot
    :open="open"
    :modal="false"
    @update:open="emit('update:open', $event)"
    ><DropdownMenuTrigger as-child><slot name="trigger" /></DropdownMenuTrigger
    ><DropdownMenuPortal v-if="portal"
      ><DropdownMenuContent
        class="ui-popup ui-menu"
        :aria-label="label"
        :side-offset="6"
        ><template v-for="(item, index) in items" :key="item.id"
          ><template v-if="item.group && item.group !== items[index - 1]?.group"
            ><DropdownMenuSeparator
              v-if="index"
              class="ui-divider"
            /><DropdownMenuLabel>{{ item.group }}</DropdownMenuLabel></template
          ><DropdownMenuItem
            class="ui-option"
            :disabled="item.disabled"
            @select="emit('select', item.id)"
            >{{ item.label }}</DropdownMenuItem
          ></template
        ></DropdownMenuContent
      ></DropdownMenuPortal
    ></DropdownMenuRoot
  >
</template>
