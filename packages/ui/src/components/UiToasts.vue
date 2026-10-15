<script setup lang="ts">
import {
  ToastProvider,
  ToastViewport,
  ToastRoot,
  ToastTitle,
  ToastClose,
} from "reka-ui";
import { useUiContext } from "../context";
import type { UiTone } from "./feedback";
defineProps<{
  items: readonly { id: string; text: string; tone?: UiTone }[];
}>();
const emit = defineEmits<{ dismiss: [id: string] }>();
const { portal } = useUiContext();
</script>
<template>
  <ToastProvider :duration="5000" label="消息"
    ><Teleport v-if="portal" :to="portal"
      ><ToastRoot
        v-for="item in items"
        :key="item.id"
        :open="true"
        class="ui-toast"
        :class="`ui-feedback-${item.tone ?? 'neutral'}`"
        @update:open="!$event && emit('dismiss', item.id)"
        ><ToastTitle>{{ item.text }}</ToastTitle
        ><ToastClose aria-label="关闭消息">×</ToastClose></ToastRoot
      ><ToastViewport class="ui-toast-viewport" label="消息通知" /></Teleport
  ></ToastProvider>
</template>
