<script setup lang="ts">
import { computed, ref } from "vue";
import { parseDate } from "@internationalized/date";
import {
  DatePickerRoot,
  DatePickerField,
  DatePickerInput,
  DatePickerTrigger,
  DatePickerContent,
  DatePickerCalendar,
  DatePickerHeader,
  DatePickerPrev,
  DatePickerNext,
  DatePickerHeading,
  DatePickerGrid,
  DatePickerGridHead,
  DatePickerGridBody,
  DatePickerGridRow,
  DatePickerHeadCell,
  DatePickerCell,
  DatePickerCellTrigger,
} from "reka-ui";
import { useUiContext } from "../context";
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
const { portal } = useUiContext();
function date(value?: string) {
  try {
    return value ? parseDate(value) : undefined;
  } catch {
    return undefined;
  }
}
const value = computed(() => date(props.modelValue));
const control = ref<InstanceType<typeof DatePickerField>>();
const focus = () =>
  (control.value?.$el as HTMLElement)
    ?.querySelector<HTMLElement>('[role="spinbutton"]')
    ?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <DatePickerRoot
    :model-value="value"
    :min-value="date(min)"
    :max-value="date(max)"
    :disabled="disabled"
    :readonly="readonly"
    :close-on-select="true"
    @update:model-value="emit('update:modelValue', $event?.toString() ?? '')"
  >
    <DatePickerField
      ref="control"
      v-slot="{ segments }"
      v-bind="{ ...field, ...$attrs }"
      class="ui-date-field"
      ><DatePickerInput
        v-for="segment in segments"
        :key="segment.part"
        :part="segment.part"
        >{{ segment.value }}</DatePickerInput
      ><DatePickerTrigger aria-label="打开日历" :disabled="disabled || readonly"
        >▦</DatePickerTrigger
      ></DatePickerField
    >
    <DatePickerContent class="ui-popup" :side-offset="4"
      ><DatePickerCalendar v-slot="{ grid, weekDays }"
        ><DatePickerHeader class="ui-calendar-header"
          ><DatePickerPrev aria-label="上个月">‹</DatePickerPrev
          ><DatePickerHeading /><DatePickerNext aria-label="下个月"
            >›</DatePickerNext
          ></DatePickerHeader
        ><DatePickerGrid v-for="month in grid" :key="month.value.toString()"
          ><DatePickerGridHead
            ><DatePickerGridRow
              ><DatePickerHeadCell v-for="day in weekDays" :key="day">{{
                day
              }}</DatePickerHeadCell></DatePickerGridRow
            ></DatePickerGridHead
          ><DatePickerGridBody
            ><DatePickerGridRow v-for="(week, i) in month.rows" :key="i"
              ><DatePickerCell
                v-for="day in week"
                :key="day.toString()"
                :date="day"
                ><DatePickerCellTrigger
                  :day="day"
                  :month="
                    month.value
                  " /></DatePickerCell></DatePickerGridRow></DatePickerGridBody></DatePickerGrid></DatePickerCalendar
    ></DatePickerContent>
  </DatePickerRoot>
</template>
