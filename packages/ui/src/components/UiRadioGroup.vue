<script setup lang="ts">
import { ref } from "vue";
import { RadioGroupRoot, RadioGroupItem, RadioGroupIndicator } from "reka-ui";
import { useFieldControl } from "../field";
import type { UiOption } from "../context";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  options: readonly UiOption[];
  disabled?: boolean;
  readonly?: boolean;
  label: string;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const root = ref<InstanceType<typeof RadioGroupRoot>>();
const focus = () =>
  (root.value?.$el as HTMLElement)
    ?.querySelector<HTMLElement>(
      '[role="radio"][data-state="checked"], [role="radio"]:not([disabled])',
    )
    ?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <RadioGroupRoot
    ref="root"
    v-bind="{ ...field, ...$attrs }"
    class="ui-radio-group"
    :aria-label="label"
    :model-value="modelValue ?? null"
    :disabled="disabled"
    :aria-readonly="readonly || undefined"
    @update:model-value="
      !props.readonly && emit('update:modelValue', String($event))
    "
  >
    <label v-for="option in options" :key="option.value" class="ui-radio-label"
      ><RadioGroupItem
        :value="option.value"
        :disabled="option.disabled"
        class="ui-radio"
        ><RadioGroupIndicator class="ui-radio-dot" /></RadioGroupItem
      >{{ option.label }}</label
    >
  </RadioGroupRoot>
</template>
