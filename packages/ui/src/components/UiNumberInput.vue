<script setup lang="ts">
import { ref } from "vue";
import { NumberFieldRoot, NumberFieldInput, NumberFieldIncrement, NumberFieldDecrement } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = withDefaults(defineProps<{ modelValue: number | null; min?: number; max?: number; step?: number; unit?: string; disabled?: boolean; readonly?: boolean }>(), { step: 1 });
const emit = defineEmits<{ "update:modelValue": [value: number | null] }>();
const input = ref<InstanceType<typeof NumberFieldInput>>();
const focus = () => (input.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <NumberFieldRoot class="ui-number" :model-value="modelValue ?? NaN" :min="min" :max="max" :step="step" :disabled="disabled" :readonly="readonly" @update:model-value="emit('update:modelValue', Number.isFinite($event) ? $event : null)">
    <NumberFieldInput ref="input" v-bind="{ ...field, ...$attrs }" class="ui-input" />
    <span v-if="unit" class="ui-unit">{{ unit }}</span>
    <NumberFieldDecrement aria-label="减少" :disabled="disabled || readonly">−</NumberFieldDecrement>
    <NumberFieldIncrement aria-label="增加" :disabled="disabled || readonly">+</NumberFieldIncrement>
  </NumberFieldRoot>
</template>
