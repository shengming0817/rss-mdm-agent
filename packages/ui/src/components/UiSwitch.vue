<script setup lang="ts">
import { ref } from "vue";
import { SwitchRoot, SwitchThumb } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{ modelValue: boolean; disabled?: boolean; readonly?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();
const control = ref<{ $el: HTMLElement }>();
const focus = () => (control.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <SwitchRoot ref="control" v-bind="{ ...field, ...$attrs }" class="ui-switch" :model-value="modelValue" :disabled="disabled" :aria-readonly="readonly || undefined" @update:model-value="!props.readonly && emit('update:modelValue', $event)"><SwitchThumb class="ui-switch-thumb" /></SwitchRoot>
</template>
