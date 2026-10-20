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
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  collisionPadding?: number;
  items: readonly (UiItem & { group?: string })[];
}>();
const emit = defineEmits<{
  "update:open": [value: boolean];
  select: [id: string];
  closeAutoFocus: [event: Event];
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
        :side="side"
        :align="align"
        :collision-padding="collisionPadding"
        :side-offset="6"
        @close-auto-focus="emit('closeAutoFocus', $event)"
        ><slot name="header" /><template
          v-for="(item, index) in items"
          :key="item.id"
          ><template v-if="item.group && item.group !== items[index - 1]?.group"
            ><DropdownMenuSeparator
              v-if="index"
              class="ui-divider"
            /><DropdownMenuLabel class="ui-menu-label">{{
              item.group
            }}</DropdownMenuLabel></template
          ><DropdownMenuItem
            class="ui-option"
            :disabled="item.disabled"
            @select="emit('select', item.id)"
            ><slot name="item" :item="item">{{
              item.label
            }}</slot></DropdownMenuItem
          ></template
        ></DropdownMenuContent
      ></DropdownMenuPortal
    ></DropdownMenuRoot
  >
</template>
