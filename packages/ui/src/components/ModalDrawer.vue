<script setup lang="ts">
import { X } from "@lucide/vue";
import { onMounted, onBeforeUnmount, ref } from "vue";
defineProps<{ label: string; side: "left" | "right" }>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
// ref: WAI-ARIA APG dialog-modal keyboard interaction. WebKit's native Tab
// sequence depends on macOS keyboard settings; keep this modal sequence explicit.
function keys(event: KeyboardEvent) {
  if (
    event.defaultPrevented ||
    (event.target instanceof HTMLElement &&
      event.target.closest("dialog") !== dialog.value) ||
    event.key !== "Tab" ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey
  )
    return;
  const controls = [
    ...(dialog.value?.querySelectorAll<HTMLElement>(
      "button,input,select,textarea,summary,[tabindex]",
    ) ?? []),
  ].filter(
    (el) =>
      el.tabIndex >= 0 &&
      !el.matches(":disabled") &&
      !el.closest('[inert],[aria-hidden="true"]') &&
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
  dialog.value?.querySelector<HTMLElement>("[autofocus]")?.focus();
});
onBeforeUnmount(() => {
  dialog.value?.close();
});
</script>
<template>
  <dialog
    ref="dialog"
    class="rss-ui drawer"
    :class="side"
    :aria-label="label"
    @cancel.prevent="emit('close')"
    @keydown="keys"
  >
    <header>
      <h2>{{ label }}</h2>
      <button
        autofocus
        type="button"
        class="icon-button"
        :aria-label="'关闭' + label"
        @click="emit('close')"
      >
        <X :size="20" aria-hidden="true" />
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
  width: min(var(--rss-drawer-width), 100vw);
  max-width: 100vw;
  height: 100%;
  max-height: 100%;
  box-sizing: border-box;
  padding: var(--rss-drawer-padding);
  overflow: auto;
  border: 0;
  border-left: 1px solid var(--rss-color-border);
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
}
.drawer.left {
  inset: 0 auto 0 0;
  border-left: 0;
  border-right: 1px solid var(--rss-color-border);
}
.drawer::backdrop {
  background: #0006;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}
h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
</style>
