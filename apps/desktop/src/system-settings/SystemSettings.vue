<script setup lang="ts">
import { reactive, computed, nextTick, ref } from "vue";
import { ChevronRight, UiSearchInput, useUiContext } from "@rss-mdm-agent/ui";
import SettingsGlyph from "./SettingsGlyph.vue";
import { systemEntries } from "./catalog";
import type { PageNavigation } from "../workspace/page-navigation";
const emit = defineEmits<{
  navigation: [state: PageNavigation];
  home: [];
}>();
const { configuration } = useUiContext();
const mac = computed(() => configuration.platform === "macos");
const query = ref("");
const selected = ref("overview");
const heading = ref<HTMLElement>();
const searchInput = ref<InstanceType<typeof UiSearchInput>>();
const listing = ref<HTMLElement>();
const entries = computed(() => systemEntries(mac.value));
const sections = computed(() => [
  { id: "overview", title: "系统概览" },
  ...entries.value.filter((e) => e.id !== "lock"),
]);
const current = computed(() =>
  entries.value.find((e) => e.id === selected.value),
);
const filtered = computed(() =>
  sections.value.filter(
    (e) =>
      e.id !== "overview" &&
      `${e.title}${entries.value.find((a) => a.id === e.id)?.description}`.includes(
        query.value.trim(),
      ),
  ),
);
const section = computed(() =>
  selected.value === "lock" ? "power" : selected.value,
);
const path = computed(() =>
  selected.value === "overview"
    ? []
    : [
        ...(selected.value === "lock"
          ? [
              {
                id: "power",
                title: entries.value.find((e) => e.id === "power")!.title,
              },
            ]
          : []),
        { id: selected.value, title: current.value!.title },
      ],
);
let origin = "";
async function openSection(id: string) {
  if (!sections.value.some((e) => e.id === id) && id !== "lock") return;
  selected.value = id;
  await nextTick();
  heading.value?.focus();
}
function reset() {
  query.value = "";
  selected.value = "overview";
}
async function inspect(id: string) {
  origin = id;
  await openSection(id);
}
async function back() {
  selected.value = "overview";
  await nextTick();
  const button = Array.from(
    listing.value?.querySelectorAll<HTMLButtonElement>(
      "button[data-setting]",
    ) ?? [],
  ).find((e) => e.dataset.setting === origin);
  if (button) button.focus();
  else searchInput.value?.focus();
}

emit(
  "navigation",
  reactive({
    reset,
    openSection,
    section,
    sections,
    path,
    resources: computed(() =>
      (current.value ? [current.value] : filtered.value).map((item) => ({
        id: item.id,
        title: item.title,
        description:
          entries.value.find((e) => e.id === item.id)?.description ?? "",
        status: "系统值未读取",
        source: "系统设置目录",
      })),
    ),
    setQuery: (value: string) => (query.value = value),
  }),
);
</script>
<template>
  <section class="system-settings design-page" aria-label="系统设置工作区">
    <template v-if="!current"
      ><header class="setting-detail-heading">
        <div>
          <h2>系统设置</h2>
          <p>从网络连接到屏幕显示，让电脑更合你的习惯。</p>
        </div>
        <small>{{ mac ? "macOS" : "Windows" }}</small>
      </header>
      <div class="design-hero system-hero">
        <div>
          <span class="design-eyebrow">MAKE IT YOURS</span>
          <h2>让好用的电脑，<br />更懂你的习惯。</h2>
          <p>网络、显示与日常偏好。</p>
          <button type="button" class="design-button" @click="inspect('about')">
            了解这台电脑 →
          </button>
        </div>
        <div class="system-hero-art" aria-hidden="true">
          <SettingsGlyph kind="general" :size="62" /><span
            ><SettingsGlyph kind="display" :size="28"
          /></span>
        </div>
      </div>
      <div class="section-heading system-search">
        <span />
        <UiSearchInput
          ref="searchInput"
          v-model="query"
          aria-label="搜索系统设置"
          placeholder="搜索系统设置"
        />
      </div>
      <div ref="listing" class="settings-overview-grid">
        <button
          v-for="item in filtered"
          :key="item.id"
          :data-setting="item.id"
          type="button"
          @click="inspect(item.id)"
        >
          <span class="design-symbol"
            ><SettingsGlyph :kind="item.id" :size="25" /></span
          ><strong>{{ item.title }}</strong
          ><span>{{ entries.find((e) => e.id === item.id)?.description }}</span
          ><ChevronRight :size="17" aria-hidden="true" />
        </button>
      </div>
      <p v-if="!filtered.length" role="status">
        没有匹配的设置，请换个关键词。
      </p>
      <button class="text-link computer-return" @click="emit('home')">
        返回我的电脑 →
      </button>
    </template>
    <template v-else>
      <header class="setting-detail-heading">
        <div>
          <h2 ref="heading" tabindex="-1">{{ current.title }}</h2>
          <p>{{ current.description }}</p>
        </div>
        <small>{{ mac ? "macOS" : "Windows" }}</small>
      </header>
      <section
        v-for="(group, index) in selected === 'lock'
          ? [
              { title: '屏幕闲置', rows: current.rows.slice(0, 2) },
              { title: '返回电脑时', rows: current.rows.slice(2) },
            ]
          : [{ title: '状态与偏好', rows: current.rows }]"
        :key="group.title"
      >
        <h3>{{ group.title }}</h3>
        <div class="design-rows">
          <div
            v-for="row in group.rows"
            :key="row"
            class="design-row setting-row"
          >
            <strong>{{ row }}</strong
            ><select disabled :aria-label="row">
              <option>未读取</option>
            </select>
          </div>
          <button
            v-if="selected === 'power' && index === 0"
            class="design-row"
            type="button"
            data-setting="lock"
            @click="openSection('lock')"
          >
            <strong>{{ entries.find((e) => e.id === "lock")!.title }}</strong
            ><ChevronRight :size="18" aria-hidden="true" />
          </button>
        </div>
      </section>
      <p class="design-note">{{ current.note }}</p>
      <button class="text-link settings-back" type="button" @click="back">
        ‹ 返回设置概览
      </button>
    </template>
    <p class="design-note connection-note">
      系统设置尚未接入。此处不会读取或修改你的系统配置。
    </p>
  </section>
</template>
<style scoped>
.system-hero {
  min-height: 310px;
}
.device-illustration {
  width: 40%;
  max-width: 370px;
  margin-right: 4%;
  transform: rotate(-7deg);
  border-bottom: 9px solid #98a8aa;
  border-radius: 4px;
}
.device-illustration > div {
  display: grid;
  place-content: center;
  gap: 10px;
  text-align: center;
  aspect-ratio: 1.5;
  border: 7px solid #445151;
  border-radius: 14px 14px 4px 4px;
  background: linear-gradient(145deg, #91a4b4, #e2ccb0);
  color: #fff;
}
.device-illustration span {
  font-size: 48px;
  font-weight: 300;
}
.device-illustration small {
  opacity: 0.8;
}
.settings-overview-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}
.settings-overview-grid button {
  position: relative;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) 18px;
  align-items: center;
  gap: 8px 14px;
  padding: 23px;
  border: 1px solid var(--rss-color-border);
  border-radius: 15px;
  text-align: left;
  color: var(--rss-color-text);
  background: var(--rss-color-bg);
  cursor: pointer;
}
.design-symbol {
  grid-row: span 2;
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: #e9eeed;
  color: #6f9186;
}
.settings-overview-grid button > span:not(.design-symbol) {
  grid-column: 2;
  font-size: 12px;
  color: var(--rss-color-text-muted);
  line-height: 1.5;
}
.settings-overview-grid button > svg {
  grid-column: 3;
  grid-row: 1 / span 2;
  color: var(--rss-color-text-muted);
}
.setting-detail-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 20px;
  margin: 12px 0 36px;
}
.setting-detail-heading h2 {
  font-size: 28px;
  margin: 0 0 10px;
}
.setting-detail-heading p,
.setting-detail-heading small {
  color: var(--rss-color-text-muted);
}
.setting-detail-heading p {
  font-size: 14px;
}
@media (max-width: 1100px) {
  .settings-overview-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 600px) {
  .settings-overview-grid {
    grid-template-columns: 1fr;
  }
  .device-illustration {
    display: none;
  }
}
</style>
