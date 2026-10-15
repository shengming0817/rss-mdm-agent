<script setup lang="ts">
import { computed, ref } from "vue";
import UiCheckbox from "./UiCheckbox.vue";
import UiCollapsible from "./UiCollapsible.vue";
import UiTable from "./UiTable.vue";
export interface ChartSeries {
  id: string;
  label: string;
  values: readonly (number | null)[];
}
const props = withDefaults(
  defineProps<{
    label: string;
    categories: readonly string[];
    series: readonly ChartSeries[];
    modelValue: string[];
    kind?: "line" | "bar";
    unit?: string;
  }>(),
  { kind: "line", unit: "" },
);
const emit = defineEmits<{ "update:modelValue": [ids: string[]] }>();
const table = ref(false);
const selected = computed(() =>
  props.series.filter((series) => props.modelValue.includes(series.id)),
);
const values = computed(() =>
  selected.value.flatMap((series) =>
    series.values
      .slice(0, props.categories.length)
      .filter((v): v is number => v !== null && Number.isFinite(v)),
  ),
);
const domain = computed(() => {
  const min = Math.min(0, ...values.value),
    max = Math.max(0, ...values.value);
  return { min, max: min === max ? min + 1 : max };
});
const y = (v: number) =>
  220 - ((v - domain.value.min) / (domain.value.max - domain.value.min)) * 180;
const x = (index: number) =>
  55 + ((index + 0.5) / Math.max(1, props.categories.length)) * 560;
const colors = [
  "var(--rss-color-accent)",
  "var(--rss-color-success)",
  "var(--rss-color-warn)",
  "var(--rss-color-danger)",
];
const color = (index: number) => colors[index % 4];
function path(series: ChartSeries) {
  let started = false;
  return props.categories
    .map((_, index) => {
      const value = series.values[index];
      if (value == null || !Number.isFinite(value)) {
        started = false;
        return "";
      }
      const command = `${started ? "L" : "M"}${x(index)},${y(value)}`;
      started = true;
      return command;
    })
    .join(" ");
}
const rows = computed(() =>
  props.categories.map((label, index) => ({
    id: String(index),
    label,
    ...Object.fromEntries(
      selected.value.map((series, seriesIndex) => [
        `series-${seriesIndex}`,
        Number.isFinite(series.values[index])
          ? `${series.values[index]}${props.unit}`
          : "—",
      ]),
    ),
  })),
);
const columns = computed(() => [
  { key: "label", label: "类别" },
  ...selected.value.map((series, index) => ({
    key: `series-${index}`,
    label: series.label,
  })),
]);
</script>
<template>
  <figure class="ui-chart" :aria-label="label">
    <figcaption>{{ label }}</figcaption>
    <div class="ui-chart-legend">
      <label v-for="(series, index) in series" :key="series.id"
        ><UiCheckbox
          :model-value="modelValue.includes(series.id)"
          @update:model-value="
            emit(
              'update:modelValue',
              $event
                ? [...modelValue, series.id]
                : modelValue.filter((id) => id !== series.id),
            )
          "
        /><span
          :style="{
            borderColor: color(index),
            borderStyle: index % 2 ? 'dashed' : 'solid',
          }"
          aria-hidden="true"
        />{{ series.label }}</label
      >
    </div>
    <svg
      v-if="values.length"
      viewBox="0 0 640 260"
      role="img"
      :aria-label="`${label}，${kind === 'line' ? '折线图' : '柱状图'}，下方可展开数据表`"
    >
      <line x1="45" x2="625" :y1="y(0)" :y2="y(0)" stroke="currentColor" />
      <text x="4" y="35">{{ domain.max.toFixed(1) }}{{ unit }}</text>
      <text x="4" y="225">{{ domain.min.toFixed(1) }}</text>
      <template v-for="(series, index) in selected" :key="series.id">
        <path
          v-if="kind === 'line'"
          :d="path(series)"
          fill="none"
          :stroke="color(props.series.indexOf(series))"
          stroke-width="2.5"
          :stroke-dasharray="index % 2 ? '7 4' : undefined"
        />
        <template v-for="(_, position) in categories" :key="position">
          <circle
            v-if="
              kind === 'line' &&
              series.values[position] != null &&
              Number.isFinite(series.values[position])
            "
            :cx="x(position)"
            :cy="y(series.values[position]!)"
            r="3.5"
            :fill="color(props.series.indexOf(series))"
          />
          <rect
            v-if="
              kind === 'bar' &&
              series.values[position] != null &&
              Number.isFinite(series.values[position])
            "
            :x="
              x(position) -
              200 / Math.max(1, categories.length) +
              (index * 400) / Math.max(1, categories.length * selected.length)
            "
            :y="Math.min(y(0), y(series.values[position]!))"
            :height="Math.abs(y(series.values[position]!) - y(0))"
            :width="360 / Math.max(1, categories.length * selected.length)"
            :fill="color(props.series.indexOf(series))"
          />
        </template>
      </template>
      <text
        v-for="(label, index) in categories"
        :key="index"
        :x="x(index)"
        y="248"
        text-anchor="middle"
      >
        {{ label }}
      </text>
    </svg>
    <p v-else role="status">暂无可显示数据</p>
    <UiCollapsible v-model:open="table" label="查看数据表"
      ><UiTable :label="`${label}数据表`" :columns="columns" :rows="rows"
    /></UiCollapsible>
  </figure>
</template>
