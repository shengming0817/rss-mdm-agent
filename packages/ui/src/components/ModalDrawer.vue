<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
defineProps<{ label: string }>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
// ref: WAI-ARIA APG dialog-modal keyboard interaction. WebKit's native Tab
// sequence depends on macOS keyboard settings; keep this modal sequence explicit.
function keys(event: KeyboardEvent) {
  if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey)
    return;
  const controls = [
    ...(dialog.value?.querySelectorAll<HTMLElement>(
      "button,input,select,textarea,summary,[tabindex]",
    ) ?? []),
  ].filter(
    (el) =>
      el.tabIndex >= 0 &&
      !el.matches(":disabled") &&
      el.getClientRects().length,
  );
  event.preventDefault();
  const index = controls.findIndex((el) => el === event.target);
  const next =
    index < 0
      ? event.shiftKey
        ? controls.length - 1
        : 0
      : (index + (event.shiftKey ? -1 : 1) + controls.length) % controls.length;
  controls.at(next)?.focus();
}
onMounted(() => {
  dialog.value?.showModal();
});
onBeforeUnmount(() => {
  dialog.value?.close();
});
</script>
<template>
  <dialog
    ref="dialog"
    class="rss-ui drawer"
    :aria-label="label"
    @cancel.prevent="emit('close')"
    @keydown="keys"
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
