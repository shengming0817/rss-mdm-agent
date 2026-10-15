<script setup lang="ts">
import { ref } from "vue";
import {
  ComboboxRoot,
  ComboboxAnchor,
  ComboboxInput,
  ComboboxTrigger,
  ComboboxPortal,
  ComboboxContent,
  ComboboxViewport,
  ComboboxItem,
  ComboboxItemIndicator,
  ComboboxEmpty,
} from "reka-ui";
import { useUiContext, type UiOption } from "../context";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  query: string;
  options: readonly UiOption[];
  disabled?: boolean;
  readonly?: boolean;
  placeholder?: string;
}>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  "update:query": [value: string];
}>();
const { portal } = useUiContext();
const input = ref<InstanceType<typeof ComboboxInput>>();
const focus = () => (input.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <ComboboxRoot
    :model-value="modelValue ?? null"
    :disabled="disabled || readonly"
    :ignore-filter="true"
    @update:model-value="
      !props.readonly && emit('update:modelValue', String($event ?? ''))
    "
  >
    <ComboboxAnchor class="ui-combobox"
      ><ComboboxInput
        ref="input"
        v-bind="{ ...field, ...$attrs }"
        class="ui-input"
        :model-value="query"
        :placeholder="placeholder"
        :display-value="
          (value) => options.find((o) => o.value === value)?.label ?? ''
        "
        @update:model-value="emit('update:query', $event)"
      /><ComboboxTrigger aria-label="显示选项"
        >⌄</ComboboxTrigger
      ></ComboboxAnchor
    >
    <ComboboxPortal v-if="portal"
      ><ComboboxContent
        class="ui-popup ui-options"
        position="popper"
        :side-offset="4"
        ><ComboboxViewport
          ><ComboboxEmpty>无匹配选项</ComboboxEmpty
          ><ComboboxItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            :disabled="option.disabled"
            class="ui-option"
            ><ComboboxItemIndicator aria-hidden="true">✓</ComboboxItemIndicator
            >{{ option.label }}</ComboboxItem
          ></ComboboxViewport
        ></ComboboxContent
      ></ComboboxPortal
    >
  </ComboboxRoot>
</template>
