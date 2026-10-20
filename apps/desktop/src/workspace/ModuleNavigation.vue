<script setup lang="ts">
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";
defineProps<{
  items: { id: string; title: string; kind?: string; group?: string }[];
  selected: string;
}>();
const emit = defineEmits<{ select: [id: string] }>();
</script>
<template>
  <nav v-if="items.length" class="module-navigation" aria-label="当前页面分类">
    <template v-for="(item, index) in items" :key="item.id">
      <span
        v-if="
          item.group &&
          (index === 0 || items.at(index - 1)?.group !== item.group)
        "
        class="module-group"
        :aria-label="item.group"
        >{{ item.group }}</span
      >
      <button
        type="button"
        :title="item.title"
        :aria-label="item.title"
        :aria-current="selected === item.id ? 'page' : undefined"
        @click="emit('select', item.id)"
      >
        <SettingsGlyph :kind="item.kind ?? item.id" :size="16" /><span>{{
          item.title
        }}</span>
      </button>
    </template>
  </nav>
</template>
