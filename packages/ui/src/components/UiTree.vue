<script setup lang="ts">
import { computed } from "vue";
import { TreeRoot, TreeItem } from "reka-ui";
import type { UiTreeNode } from "../context";
const props = defineProps<{
  items: UiTreeNode[];
  modelValue: string[];
  expanded: string[];
  multiple?: boolean;
  disabled?: boolean;
  label: string;
}>();
const emit = defineEmits<{
  "update:modelValue": [ids: string[]];
  "update:expanded": [ids: string[]];
  load: [id: string];
  activate: [id: string];
}>();
const all = computed(() => {
  const flatten = (items: UiTreeNode[]): UiTreeNode[] =>
    items.flatMap((item) => [item, ...flatten(item.children ?? [])]);
  return flatten(props.items);
});
const selected = computed(() =>
  all.value.filter((n) => props.modelValue.includes(n.id)),
);
function update(nodes: UiTreeNode | UiTreeNode[]) {
  emit(
    "update:modelValue",
    (Array.isArray(nodes) ? nodes : [nodes]).filter(Boolean).map((n) => n.id),
  );
}
function expand(ids: string[]) {
  emit("update:expanded", ids);
  for (const id of ids.filter((id) => !props.expanded.includes(id))) {
    const node = all.value.find((n) => n.id === id);
    if (node?.hasChildren && node.children === undefined && !node.loading)
      emit("load", id);
  }
}
function activate(node: UiTreeNode) {
  if (!props.disabled && !node.disabled) emit("activate", node.id);
}
</script>
<template>
  <TreeRoot
    v-slot="{ flattenItems }"
    class="ui-tree"
    :items="items"
    :model-value="multiple ? selected : (selected[0] ?? null)"
    :multiple="multiple"
    :expanded="expanded"
    :get-key="(node: UiTreeNode) => node.id"
    :get-children="
      (node: UiTreeNode) => node.children ?? (node.hasChildren ? [] : undefined)
    "
    :disabled="disabled"
    :aria-label="label"
    @update:model-value="update"
    @update:expanded="expand"
  >
    <TreeItem
      v-for="item in flattenItems"
      :key="item._id"
      v-slot="{ isExpanded }"
      v-bind="item.bind"
      :disabled="item.value.disabled"
      class="ui-option"
      :style="{ paddingInlineStart: `${(item.level - 1) * 18 + 8}px` }"
      :aria-busy="item.value.loading || undefined"
      @dblclick="activate(item.value)"
      @keydown.enter="!$event.isComposing && activate(item.value)"
    >
      <span aria-hidden="true">{{
        item.hasChildren ? (isExpanded ? "⌄" : "›") : "·"
      }}</span
      ><slot name="item" :item="item.value">{{ item.value.label }}</slot
      ><span v-if="item.value.loading" role="status">加载中</span
      ><span v-if="item.value.error" role="alert">{{ item.value.error }}</span>
    </TreeItem>
  </TreeRoot>
  <p v-if="!items.length" role="status">暂无项目</p>
</template>
