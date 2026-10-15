<script setup lang="ts">
import { computed, ref } from "vue";
import { parseDateTime } from "@internationalized/date";
import { DateFieldRoot, DateFieldInput } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  disabled?: boolean;
  readonly?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const value = computed(() => {
  try {
    return props.modelValue ? parseDateTime(props.modelValue) : null;
  } catch {
    return null;
  }
});
const control = ref<InstanceType<typeof DateFieldRoot>>();
const focus = () =>
  (control.value?.$el as HTMLElement)
    ?.querySelector<HTMLElement>('[role="spinbutton"]')
    ?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
// Reka owns incomplete segment editing; completed values are always caller-controlled.
</script>
<template>
  <DateFieldRoot
    ref="control"
    v-slot="{ segments }"
    v-bind="{ ...field, ...$attrs }"
    class="ui-date-field"
    :model-value="value"
    :disabled="disabled"
    :readonly="readonly"
    granularity="minute"
    :hour-cycle="24"
    @update:model-value="emit('update:modelValue', $event?.toString() ?? '')"
    ><DateFieldInput
      v-for="segment in segments"
      :key="segment.part"
      :part="segment.part"
      >{{ segment.value }}</DateFieldInput
    ></DateFieldRoot
  >
</template>
