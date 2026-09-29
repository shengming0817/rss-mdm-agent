<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
defineProps<{ label: string }>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
let previous: HTMLElement | null = null;
onMounted(() => {
  previous =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  dialog.value?.showModal();
});
onBeforeUnmount(() => {
  dialog.value?.close();
  if (previous?.isConnected) previous.focus();
});
</script>
<template>
  <dialog
    ref="dialog"
    class="rss-ui drawer"
    :aria-label="label"
    @cancel.prevent="emit('close')"
  >
    <header>
      <h2>{{ label }}</h2>
      <button
        autofocus
        type="button"
        :aria-label="'关闭' + label"
        @click="emit('close')"
      >
        关闭
      </button>
    </header>
    <slot />
  </dialog>
</template>
<style scoped>
.drawer {
  position: fixed;
  inset: 0 0 0 auto;
  margin: 0;
  width: min(520px, 100vw);
  max-width: 100vw;
  height: 100%;
  max-height: 100%;
  box-sizing: border-box;
  padding: 24px;
  overflow: auto;
  border: 0;
  border-left: 1px solid var(--color-border);
  background: var(--color-bg);
  color: var(--color-text);
}
.drawer::backdrop {
  background: #0006;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
button {
  padding: 8px 12px;
}
</style>
