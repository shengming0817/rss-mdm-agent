<script setup lang="ts">
import { ref } from "vue";
import {
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectPortal,
  SelectContent,
  SelectViewport,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
} from "reka-ui";
import { useUiContext, type UiOption } from "../context";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    modelValue?: string;
    options: readonly UiOption[];
    disabled?: boolean;
    readonly?: boolean;
    placeholder?: string;
  }>(),
  { placeholder: "请选择" },
);
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const { configuration, portal } = useUiContext();
const trigger = ref<InstanceType<typeof SelectTrigger>>();
const focus = () => (trigger.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
// ref: reka-ui v2.10.5 Select/SelectContentImpl.vue — two platform positioning modes.
</script>
<template>
  <SelectRoot
    :model-value="modelValue ?? null"
    :disabled="disabled"
    @update:model-value="
      !props.readonly && emit('update:modelValue', String($event ?? ''))
    "
  >
    <SelectTrigger
      ref="trigger"
      v-bind="{ ...field, ...$attrs }"
      class="ui-select-trigger"
      :disabled="disabled || readonly"
      :aria-readonly="readonly || undefined"
      ><SelectValue :placeholder="placeholder" /><span aria-hidden="true"
        >⌄</span
      ></SelectTrigger
    >
    <SelectPortal v-if="portal"
      ><SelectContent
        class="ui-popup ui-options"
        :position="
          configuration.platform === 'macos' ? 'item-aligned' : 'popper'
        "
        :side-offset="4"
        ><SelectViewport>
          <SelectItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            :disabled="option.disabled"
            class="ui-option"
            ><SelectItemIndicator aria-hidden="true">✓</SelectItemIndicator
            ><SelectItemText>{{ option.label }}</SelectItemText></SelectItem
          >
          <p v-if="!options.length" role="status">暂无选项</p>
        </SelectViewport></SelectContent
      ></SelectPortal
    >
  </SelectRoot>
</template>
