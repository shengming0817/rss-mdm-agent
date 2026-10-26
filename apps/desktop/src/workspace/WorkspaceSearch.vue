<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from "vue";
import {
  UiPopover,
  UiSearchInput,
  Search,
  ArrowUpRight,
} from "@rss-mdm-agent/ui";
import type { SearchEntry, NavigationTarget } from "./page-navigation";
const props = defineProps<{
  request: number;
  entries: SearchEntry[];
  loading: boolean;
  complete: boolean;
  hasMore: boolean;
  error: string;
}>();
const emit = defineEmits<{
  select: [entry: NavigationTarget];
  more: [];
  retry: [];
  visibility: [visible: boolean];
}>();
const searchOpen = ref(false),
  query = ref(""),
  active = ref(0);
const input = ref<InstanceType<typeof UiSearchInput>>();
const results = ref<HTMLElement>();
const listId = `workspace-search-${useId()}`;
const filtered = computed(() =>
  props.entries.filter((e) =>
    `${e.group}${e.title}`
      .toLocaleLowerCase()
      .includes(query.value.trim().toLocaleLowerCase()),
  ),
);
function choose(entry = filtered.value.at(active.value)) {
  if (!entry) return;
  searchOpen.value = false;
  emit("select", entry);
}
function enter(event: KeyboardEvent) {
  if (!event.isComposing) choose();
}
async function move(direction: number) {
  if (!filtered.value.length) return;
  active.value =
    (active.value + direction + filtered.value.length) % filtered.value.length;
  await nextTick();
  results.value
    ?.querySelector('[aria-selected="true"]')
    ?.scrollIntoView({ block: "nearest" });
}
watch(
  () => props.request,
  () => {
    searchOpen.value = true;
  },
);
watch(
  () => props.entries,
  () => {
    active.value = Math.min(
      active.value,
      Math.max(0, filtered.value.length - 1),
    );
  },
);
watch(query, () => {
  active.value = 0;
});
watch(searchOpen, async (open) => {
  emit("visibility", open);
  if (!open) return;
  query.value = "";
  active.value = 0;
  await nextTick();
  input.value?.focus();
});
</script>
<template>
  <UiPopover
    v-model:open="searchOpen"
    label="搜索应用、工具与设置"
    content-class="workspace-search-popover"
  >
    <template #trigger>
      <button
        class="workspace-search-trigger"
        type="button"
        aria-label="搜索应用、工具与设置"
      >
        <Search :size="16" aria-hidden="true" />
        <span>搜索应用、工具与设置</span><kbd>⌘ / Ctrl K</kbd>
      </button>
    </template>
    <div
      class="workspace-search-panel"
      @keydown.down.prevent="move(1)"
      @keydown.up.prevent="move(-1)"
    >
      <div class="workspace-search-field">
        <Search :size="19" aria-hidden="true" />
        <UiSearchInput
          ref="input"
          v-model="query"
          aria-label="全局搜索"
          placeholder="搜索应用、工具或设置…"
          role="combobox"
          aria-expanded="true"
          aria-autocomplete="list"
          :aria-controls="listId"
          :aria-activedescendant="
            filtered.length ? `${listId}-${active}` : undefined
          "
          @keydown.enter="enter"
        />
        <kbd>Esc</kbd>
      </div>
      <div
        :id="listId"
        ref="results"
        class="workspace-search-results"
        role="listbox"
        aria-label="搜索结果"
      >
        <template
          v-for="(entry, index) in filtered"
          :key="`${entry.page}:${entry.id}`"
        >
          <p
            v-if="index === 0 || entry.group !== filtered.at(index - 1)?.group"
            class="workspace-search-group"
            role="presentation"
          >
            {{ entry.group }}
          </p>
          <button
            :id="`${listId}-${index}`"
            type="button"
            role="option"
            tabindex="-1"
            :aria-selected="active === index"
            @pointermove="active = index"
            @click="choose(entry)"
          >
            <span>{{ entry.title }}</span
            ><ArrowUpRight :size="15" aria-hidden="true" />
          </button>
        </template>
        <p v-if="!filtered.length" class="workspace-search-empty" role="status">
          {{
            complete
              ? "没有找到匹配的入口，试试其他关键词。"
              : "当前加载范围没有匹配项，可继续查找。"
          }}
        </p>
      </div>
      <p role="status">
        {{
          loading
            ? "正在读取授权目录…"
            : complete
              ? "当前可搜索目录已读取完整"
              : "搜索仅覆盖当前加载范围"
        }}
      </p>
      <p v-if="error" role="alert">
        {{ error
        }}<button :disabled="loading" @click="emit('retry')">重试读取</button>
      </p>
      <button v-if="hasMore" :disabled="loading" @click="emit('more')">
        继续查找
      </button>
      <footer class="workspace-search-hint">
        <span><kbd>↑</kbd> <kbd>↓</kbd> 选择</span><span><kbd>↵</kbd> 打开</span
        ><span>点击外部或按 Esc 关闭</span>
      </footer>
    </div>
  </UiPopover>
</template>
<style scoped>
.workspace-search-trigger {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 34px;
  padding: 0 12px;
  border: 1px solid var(--rss-color-border-strong);
  border-radius: 9px;
  background: var(--rss-color-surface);
  box-shadow: 0 1px 2px #18223005;
  text-align: left;
  color: var(--rss-color-text-muted);
  font-size: 12px;
  cursor: text;
}
.workspace-search-trigger:hover {
  border-color: var(--rss-color-text-muted);
}
.workspace-search-trigger:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: 2px;
}
.workspace-search-trigger > span {
  flex: 1;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.workspace-search-trigger > svg {
  flex: none;
}
kbd {
  padding: 2px 4px;
  border: 1px solid var(--rss-color-border);
  border-radius: 4px;
  font-size: 10px;
  color: var(--rss-color-text-muted);
  font-family: inherit;
  white-space: nowrap;
}
:global(.ui-popover.workspace-search-popover) {
  width: min(560px, calc(100vw - 24px));
  padding: 0;
  overflow: hidden;
  border-radius: 12px;
  box-shadow: 0 12px 40px #18223020;
}
.workspace-search-field {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--rss-color-border);
  color: var(--rss-color-text-muted);
}
.workspace-search-field :deep(.ui-search) {
  flex: 1;
  min-width: 0;
}
.workspace-search-field :deep(input) {
  width: 100%;
  min-width: 0;
  border: 0;
  border-radius: 0;
  padding: 0;
  background: transparent;
  box-shadow: none;
  font-size: 14px;
  outline: none;
}
.workspace-search-results {
  max-height: min(420px, 48vh);
  overflow: auto;
  padding: 6px;
  scrollbar-width: thin;
}
.workspace-search-group {
  margin: 14px 10px 6px;
  color: var(--rss-color-text-muted);
  font-size: 11px;
}
.workspace-search-results button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  width: 100%;
  min-height: 38px;
  padding: 8px 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--rss-color-text);
  text-align: left;
  font-size: 13px;
}
.workspace-search-results button[aria-selected="true"] {
  background: var(--rss-color-accent-bg);
  color: var(--rss-color-accent);
}
.workspace-search-empty {
  margin: 24px 16px;
  color: var(--rss-color-text-muted);
  font-size: 13px;
}
.workspace-search-hint {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 16px;
  border-top: 1px solid var(--rss-color-border);
  color: var(--rss-color-text-muted);
  font-size: 10px;
}
.workspace-search-hint > span:last-child {
  margin-left: auto;
}
@media (max-width: 700px) {
  .workspace-search-trigger kbd {
    display: none;
  }
}
</style>
