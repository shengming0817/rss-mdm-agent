<script setup lang="ts">
import type { UiItem } from "../context";
defineProps<{
  items: readonly UiItem[];
  modelValue?: string;
  label: string;
  breadcrumbs?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [id: string] }>();
</script>
<template>
  <nav
    :aria-label="label"
    class="ui-navigation"
    :class="{ 'ui-breadcrumbs': breadcrumbs }"
  >
    <ol>
      <li v-for="item in items" :key="item.id">
        <button
          type="button"
          :disabled="item.disabled"
          :aria-current="modelValue === item.id ? 'page' : undefined"
          @click="emit('update:modelValue', item.id)"
        >
          <slot name="icon" :item="item" /><span>{{ item.label }}</span
          ><slot name="trailing" :item="item" />
        </button>
      </li>
    </ol>
  </nav>
</template>
