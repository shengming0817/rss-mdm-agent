<script setup lang="ts">
import { useId } from "vue";
import { UiButton } from "./primitives";
import type { UiColumn, UiRow } from "../context";
const props = withDefaults(
  defineProps<{
    columns: readonly UiColumn[];
    rows: readonly UiRow[];
    label: string;
    selection?: "none" | "single" | "multiple";
    modelValue?: string[];
    disabled?: boolean;
    /** Enable row actions; read-only tables keep ordinary table tab order. */
    activatable?: boolean;
    sort?: { key: string; direction: "ascending" | "descending" };
    page?: number;
    pageCount?: number;
  }>(),
  { selection: "none", modelValue: () => [] },
);
const emit = defineEmits<{
  "update:modelValue": [ids: string[]];
  "update:sort": [sort: { key: string; direction: "ascending" | "descending" }];
  "update:page": [page: number];
  activate: [id: string];
}>();
const id = useId();
// ref: react-aria selection/useSelectableItem.ts — action and selection are separate.
function activate(row: UiRow, event: MouseEvent | KeyboardEvent) {
  if (
    !props.activatable ||
    props.disabled ||
    row.disabled ||
    event.defaultPrevented
  )
    return;
  if (event.type === "keydown") {
    const key = event as KeyboardEvent;
    if (key.isComposing || key.repeat || event.target !== event.currentTarget)
      return;
  } else {
    const target = event.target as Element | null;
    if (
      target?.closest("tr") !== event.currentTarget ||
      target?.closest(
        "button,a,input,select,textarea,[contenteditable],[role='button'],[role='link'],[role='checkbox'],[role='switch'],[role='combobox']",
      )
    )
      return;
  }
  event.preventDefault();
  emit("activate", row.id);
}
function select(row: UiRow) {
  if (props.disabled || row.disabled) return;
  emit(
    "update:modelValue",
    props.selection === "single"
      ? [row.id]
      : props.modelValue.includes(row.id)
        ? props.modelValue.filter((id) => id !== row.id)
        : [...props.modelValue, row.id],
  );
}
</script>
<template>
  <div class="ui-table-scroll">
    <table class="ui-table">
      <caption>
        {{
          label
        }}
      </caption>
      <thead>
        <tr>
          <th v-if="selection !== 'none'" scope="col">选择</th>
          <th
            v-for="column in columns"
            :key="column.key"
            scope="col"
            :aria-sort="sort?.key === column.key ? sort.direction : undefined"
          >
            <UiButton
              v-if="column.sortable"
              variant="ghost"
              :disabled="disabled"
              @click="
                emit('update:sort', {
                  key: column.key,
                  direction:
                    sort?.key === column.key && sort.direction === 'ascending'
                      ? 'descending'
                      : 'ascending',
                })
              "
              >{{ column.label }}</UiButton
            ><template v-else>{{ column.label }}</template>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="row in rows"
          :key="row.id"
          :data-selected="modelValue.includes(row.id) || undefined"
          :tabindex="activatable && !disabled && !row.disabled ? 0 : undefined"
          :aria-disabled="disabled || row.disabled || undefined"
          @dblclick="activate(row, $event)"
          @keydown.enter.self="activate(row, $event)"
        >
          <td v-if="selection !== 'none'">
            <input
              :type="selection === 'single' ? 'radio' : 'checkbox'"
              :name="id"
              :aria-label="`选择 ${row.label}`"
              :disabled="disabled || row.disabled"
              :checked="modelValue.includes(row.id)"
              @change="select(row)"
            />
          </td>
          <td v-for="column in columns" :key="column.key">
            <slot name="cell" :row="row" :column="column">{{
              row[column.key]
            }}</slot>
          </td>
        </tr>
        <tr v-if="!rows.length">
          <td :colspan="columns.length + (selection === 'none' ? 0 : 1)">
            <slot name="empty">暂无数据</slot>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
  <nav
    v-if="page !== undefined && pageCount !== undefined"
    class="ui-pagination"
    aria-label="分页"
  >
    <UiButton
      :disabled="disabled || page <= 1"
      @click="emit('update:page', page - 1)"
      >上一页</UiButton
    ><span>{{ page }} / {{ pageCount }}</span
    ><UiButton
      :disabled="disabled || page >= pageCount"
      @click="emit('update:page', page + 1)"
      >下一页</UiButton
    >
  </nav>
</template>
