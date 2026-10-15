<script setup lang="ts">
import { computed, ref, onBeforeUnmount } from "vue";
const props = withDefaults(
  defineProps<{
    modelValue: number;
    min?: number;
    max?: number;
    direction?: "horizontal" | "vertical";
    label?: string;
  }>(),
  { min: 0.2, max: 0.8, direction: "horizontal", label: "调整分栏" },
);
const emit = defineEmits<{ "update:modelValue": [value: number] }>();
const root = ref<HTMLElement>();
const separator = ref<HTMLElement>();
let pointer: number | undefined;
const bounds = computed(() => {
  const min = Number.isFinite(props.min)
    ? Math.max(0, Math.min(1, props.min))
    : 0.2;
  return {
    min,
    max: Number.isFinite(props.max)
      ? Math.max(min, Math.min(1, props.max))
      : Math.max(min, 0.8),
  };
});
const clamp = (value: number) =>
  Math.min(
    bounds.value.max,
    Math.max(bounds.value.min, Number.isFinite(value) ? value : 0.5),
  );
const ratio = computed(() => clamp(props.modelValue));
function release() {
  if (pointer !== undefined && separator.value?.hasPointerCapture(pointer))
    separator.value.releasePointerCapture(pointer);
  pointer = undefined;
}
function down(event: PointerEvent) {
  if (event.button !== 0) return;
  pointer = event.pointerId;
  separator.value?.setPointerCapture(pointer);
}
function move(event: PointerEvent) {
  if (pointer !== event.pointerId || !root.value) return;
  const rect = root.value.getBoundingClientRect();
  const size = props.direction === "horizontal" ? rect.width : rect.height;
  if (size > 0)
    emit(
      "update:modelValue",
      clamp(
        (props.direction === "horizontal"
          ? event.clientX - rect.left
          : event.clientY - rect.top) / size,
      ),
    );
}
function key(event: KeyboardEvent) {
  const backward = props.direction === "horizontal" ? "ArrowLeft" : "ArrowUp",
    forward = props.direction === "horizontal" ? "ArrowRight" : "ArrowDown";
  if (![backward, forward, "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  emit(
    "update:modelValue",
    clamp(
      event.key === "Home"
        ? bounds.value.min
        : event.key === "End"
          ? bounds.value.max
          : ratio.value + (event.key === backward ? -0.02 : 0.02),
    ),
  );
}
onBeforeUnmount(release);
</script>
<template>
  <div
    ref="root"
    class="ui-split"
    :data-direction="direction"
    :style="{ flexDirection: direction === 'horizontal' ? 'row' : 'column' }"
  >
    <div class="ui-split-panel" :style="{ flex: `0 0 ${ratio * 100}%` }">
      <slot name="first" />
    </div>
    <div
      ref="separator"
      class="ui-split-handle"
      role="separator"
      tabindex="0"
      :aria-label="label"
      :aria-orientation="direction === 'horizontal' ? 'vertical' : 'horizontal'"
      :aria-valuenow="Math.round(ratio * 100)"
      :aria-valuemin="Math.round(bounds.min * 100)"
      :aria-valuemax="Math.round(bounds.max * 100)"
      @pointerdown="down"
      @pointermove="move"
      @pointerup="release"
      @pointercancel="release"
      @lostpointercapture="pointer = undefined"
      @keydown="key"
    />
    <div class="ui-split-panel" style="flex: 1"><slot name="second" /></div>
  </div>
</template>
