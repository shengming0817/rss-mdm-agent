<script setup lang="ts">
import { ref, watch, useId } from "vue";
import {
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
} from "reka-ui";
import { useUiContext } from "../context";
import { UiButton } from "./primitives";
const props = withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    description?: string;
    presentation?: "dialog" | "drawer";
    side?: "left" | "right";
  }>(),
  { presentation: "dialog", side: "right" },
);
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { configuration, portal } = useUiContext();
const descriptionId = `ui-dialog-description-${useId()}`;
const close = ref<{ $el: HTMLElement }>();
let returnTo: HTMLElement | undefined;
watch(
  () => props.open,
  (open) => {
    if (open && document.activeElement instanceof HTMLElement)
      returnTo = document.activeElement;
  },
  { immediate: true, flush: "sync" },
);
function restore(event: Event) {
  event.preventDefault();
  if (returnTo?.isConnected) returnTo.focus();
}
// ref: reka-ui v2.10.5 Dialog/DialogContentModal.vue — one focus owner for every modal.
</script>
<template>
  <DialogRoot :open="open" @update:open="emit('update:open', $event)"
    ><DialogPortal v-if="portal"
      ><DialogOverlay class="ui-dialog-backdrop" /><DialogContent
        class="ui-dialog"
        :aria-label="title"
        :data-presentation="presentation"
        :data-side="side"
        :data-platform="configuration.platform"
        :aria-describedby="description ? descriptionId : undefined"
        @open-auto-focus.prevent="close?.$el.focus()"
        @close-auto-focus="restore"
        @pointer-down-outside.prevent
      >
        <header>
          <DialogTitle>{{ title }}</DialogTitle
          ><DialogClose as-child
            ><UiButton
              ref="close"
              icon
              variant="ghost"
              :aria-label="`关闭${title}`"
              >×</UiButton
            ></DialogClose
          >
        </header>
        <DialogDescription v-if="description" :id="descriptionId">{{
          description
        }}</DialogDescription>
        <div class="ui-dialog-body"><slot /></div>
        <footer v-if="$slots.actions">
          <slot name="actions" />
        </footer> </DialogContent></DialogPortal
  ></DialogRoot>
</template>
