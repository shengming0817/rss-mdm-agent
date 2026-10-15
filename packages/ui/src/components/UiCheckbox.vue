<script setup lang="ts">
import { ref } from "vue";
import { CheckboxRoot, CheckboxIndicator } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue: boolean | "indeterminate";
  disabled?: boolean;
  readonly?: boolean;
}>();
const emit = defineEmits<{
  "update:modelValue": [value: boolean | "indeterminate"];
}>();
const control = ref<{ $el: HTMLElement }>();
const focus = () => (control.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <CheckboxRoot
    ref="control"
    v-bind="{ ...field, ...$attrs }"
    class="ui-check"
    :model-value="modelValue"
    :disabled="disabled"
    :aria-readonly="readonly || undefined"
    @update:model-value="!props.readonly && emit('update:modelValue', $event)"
  >
    <CheckboxIndicator>{{
      modelValue === "indeterminate" ? "−" : "✓"
    }}</CheckboxIndicator>
  </CheckboxRoot>
</template>
