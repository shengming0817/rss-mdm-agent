<script setup lang="ts">
import { computed, ref } from "vue";
import { parseTime } from "@internationalized/date";
import { TimeFieldRoot, TimeFieldInput } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  min?: string;
  max?: string;
  disabled?: boolean;
  readonly?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
function time(value?: string) {
  try {
    return value ? parseTime(value) : undefined;
  } catch {
    return undefined;
  }
}
const value = computed(() => time(props.modelValue));
const control = ref<InstanceType<typeof TimeFieldRoot>>();
const focus = () =>
  (control.value?.$el as HTMLElement)
    ?.querySelector<HTMLElement>('[role="spinbutton"]')
    ?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <TimeFieldRoot
    ref="control"
    v-slot="{ segments }"
    v-bind="{ ...field, ...$attrs }"
    class="ui-date-field"
    :model-value="value"
    :min-value="time(min)"
    :max-value="time(max)"
    :disabled="disabled"
    :readonly="readonly"
    :hour-cycle="24"
    @update:model-value="emit('update:modelValue', $event?.toString() ?? '')"
    ><TimeFieldInput
      v-for="segment in segments"
      :key="segment.part"
      :part="segment.part"
      >{{ segment.value }}</TimeFieldInput
    ></TimeFieldRoot
  >
</template>
