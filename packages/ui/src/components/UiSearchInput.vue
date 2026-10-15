<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import UiTextInput from "./UiTextInput.vue";
import { UiButton } from "./primitives";
defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{ modelValue: string; disabled?: boolean; delay?: number }>(),
  { delay: 0 },
);
const emit = defineEmits<{
  "update:modelValue": [value: string];
  search: [value: string];
}>();
const input = ref<InstanceType<typeof UiTextInput>>();
let timer: ReturnType<typeof setTimeout> | undefined;
function update(value: string) {
  emit("update:modelValue", value);
  clearTimeout(timer);
  timer = setTimeout(() => emit("search", value), Math.max(0, props.delay));
}
function submit(event: KeyboardEvent) {
  if (event.isComposing || props.disabled) return;
  event.preventDefault();
  clearTimeout(timer);
  emit("search", props.modelValue);
}
onBeforeUnmount(() => clearTimeout(timer));
defineExpose({ focus: () => input.value?.focus() });
</script>
<template>
  <div class="ui-search">
    <UiTextInput
      ref="input"
      v-bind="$attrs"
      :model-value="modelValue"
      :disabled="disabled"
      @update:model-value="update"
      @keydown.enter="submit"
    />
    <UiButton
      v-if="modelValue"
      icon
      :disabled="disabled"
      aria-label="清除搜索"
      @click="
        update('');
        input?.focus();
      "
      >×</UiButton
    >
  </div>
</template>
