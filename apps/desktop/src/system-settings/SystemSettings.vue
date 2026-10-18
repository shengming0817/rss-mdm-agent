<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import {
  ChevronRight,
  Settings,
  UiSearchInput,
  useUiContext,
} from "@rss-mdm-agent/ui";
import SettingsGlyph from "./SettingsGlyph.vue";
defineProps<{ deviceName?: string | null }>();
const emit = defineEmits<{ home: [] }>();
const { configuration } = useUiContext();
const mac = computed(() => configuration.platform === "macos");
const search = ref("");
const selected = ref("");
const heading = ref<HTMLElement>();
const list = ref<HTMLElement>();
let origin: HTMLElement | undefined;
const entries = computed(() => [
  {
    id: "network",
    title: mac.value ? "网络" : "网络和 Internet",
    description: "网络连接、服务与代理",
    color: "blue",
    group: "连接",
    rows: ["当前网络", "连接状态", mac.value ? "网络服务代理" : "当前用户代理"],
    note: mac.value
      ? "代理按所选网络服务分别配置。"
      : "用户代理与系统服务代理分别配置。",
  },
  {
    id: "display",
    title: "显示器",
    description: "屏幕排列、分辨率与缩放",
    color: "purple",
    group: "设备",
    rows: ["已连接的显示器", "分辨率", "缩放"],
    note: "显示器配置可能影响当前工作区的布局。",
  },
  {
    id: "sound",
    title: "声音",
    description: "输出设备、音量与输入",
    color: "pink",
    group: "设备",
    rows: ["声音输出", "输出音量", "声音输入"],
    note: "切换音频设备可能影响正在进行的通话。",
  },
  {
    id: "power",
    title: mac.value ? "电池" : "电源和电池",
    description: "电量、节能与供电偏好",
    color: "green",
    group: "设备",
    rows: ["电池电量", "电源状态", mac.value ? "低电量模式" : "电源模式"],
    note: "电源偏好需要分别核对电池和外接电源状态。",
  },
  {
    id: "lock",
    title: mac.value ? "锁定屏幕" : "屏幕和睡眠",
    description: "屏幕闲置与返回电脑时的行为",
    color: "cyan",
    group: "个人与安全",
    rows: ["使用电池时关闭屏幕", "接通电源时关闭屏幕", "返回时要求登录"],
    note: "关闭屏幕、系统睡眠与锁定电脑是不同设置。",
  },
  {
    id: "storage",
    title: "储存空间",
    description: "磁盘容量与空间使用情况",
    color: "orange",
    group: "设备",
    rows: ["磁盘容量", "已用空间", "可用空间"],
    note: "查看空间不会删除文件；清理前需要单独确认范围。",
  },
  {
    id: "security",
    title: mac.value ? "隐私与安全性" : "隐私和安全性",
    description: "系统保护与应用权限",
    color: "indigo",
    group: "个人与安全",
    rows: [mac.value ? "FileVault" : "设备加密", "防火墙", "应用权限"],
    note: "受组织管理的配置与系统权限需要由相应管理方确认。",
  },
  {
    id: "about",
    title: mac.value ? "通用" : "系统信息",
    description: "了解这台电脑与操作系统",
    color: "gray",
    group: "设备",
    rows: ["操作系统版本", "处理器", "内存"],
    note: "设备规格将在接入本机状态后显示。",
  },
]);
const filtered = computed(() =>
  entries.value.filter((item) =>
    `${item.title}${item.description}`.includes(search.value.trim()),
  ),
);
const current = computed(() =>
  entries.value.find((item) => item.id === selected.value),
);
async function selectCategory(id: string, event: Event) {
  origin = event.currentTarget as HTMLElement;
  selected.value = id;
  await nextTick();
  heading.value?.focus();
}
async function back() {
  selected.value = "";
  await nextTick();
  if (origin?.isConnected) origin.focus();
  else list.value?.querySelector<HTMLButtonElement>("button")?.focus();
}
</script>
<template>
  <section
    class="system-settings"
    :class="{ 'settings-mac': mac }"
    aria-label="系统设置工作区"
  >
    <div class="settings-topline">
      <button class="computer-return" type="button" @click="emit('home')">
        <span class="computer-thumbnail" aria-hidden="true" />
        <span
          ><strong>{{ deviceName || "这台电脑" }}</strong
          ><small>返回我的电脑</small></span
        >
      </button>
      <span class="preview-label">UI 预览 · 未连接系统设置</span>
    </div>
    <div class="settings-layout" :class="{ 'has-detail': current }">
      <div class="settings-directory" aria-label="设置目录">
        <div class="directory-heading">
          <h2>系统设置</h2>
          <span>{{ mac ? "macOS" : "Windows" }}</span>
        </div>
        <UiSearchInput
          v-model="search"
          aria-label="搜索系统设置"
          placeholder="搜索系统设置"
        />
        <nav ref="list" class="settings-list" aria-label="系统设置分类">
          <button
            v-for="item in filtered"
            :key="item.id"
            type="button"
            :aria-current="selected === item.id ? 'page' : undefined"
            @click="selectCategory(item.id, $event)"
          >
            <span class="setting-icon" :class="item.color"
              ><SettingsGlyph :kind="item.id" :size="19"
            /></span>
            <span class="entry-copy"
              ><strong>{{ item.title }}</strong
              ><small>{{ item.description }}</small></span
            >
            <ChevronRight :size="15" aria-hidden="true" />
          </button>
          <p v-if="!filtered.length" role="status" class="empty-search">
            没有匹配的设置，请换个关键词。
          </p>
        </nav>
      </div>
      <div class="settings-detail">
        <template v-if="current">
          <button class="settings-back" type="button" @click="back">
            ‹ 返回设置概览
          </button>
          <p class="detail-path">系统设置 / {{ current.group }}</p>
          <div class="detail-title">
            <span class="setting-icon large" :class="current.color"
              ><SettingsGlyph :kind="current.id" :size="27"
            /></span>
            <h2 ref="heading" tabindex="-1">{{ current.title }}</h2>
          </div>
          <p class="detail-description">{{ current.description }}</p>
          <div class="setting-rows">
            <div v-for="row in current.rows" :key="row" class="setting-row">
              <strong>{{ row }}</strong
              ><span>未读取</span>
            </div>
          </div>
          <p class="setting-note">{{ current.note }}</p>
          <div class="connection-note">
            <Settings :size="20" aria-hidden="true" />
            <div>
              <strong>系统设置尚未接入</strong>
              <p>
                当前可浏览设置目录和页面布局。此处不会读取或修改你的系统配置。
              </p>
            </div>
          </div>
        </template>
        <template v-else>
          <div class="settings-welcome">
            <span class="welcome-icon"
              ><Settings :size="34" aria-hidden="true"
            /></span>
            <h2>让电脑更适合你</h2>
            <p>从网络连接到屏幕显示，<br />在这里找到你需要的系统设置。</p>
          </div>
          <div class="settings-featured">
            <h3>常用设置</h3>
            <button
              v-for="item in entries.slice(0, 3)"
              :key="item.id"
              type="button"
              @click="selectCategory(item.id, $event)"
            >
              <span class="setting-icon" :class="item.color"
                ><SettingsGlyph :kind="item.id" :size="20" /></span
              ><span
                ><strong>{{ item.title }}</strong
                ><small>{{ item.description }}</small></span
              ><ChevronRight :size="16" aria-hidden="true" />
            </button>
          </div>
          <p class="overview-note">
            先选择一个分类。实际状态与可用操作将在连接系统设置后提供。
          </p>
        </template>
      </div>
    </div>
  </section>
</template>
<style scoped>
.system-settings {
  container: system-settings / inline-size;
  width: 100%;
  min-width: 0;
  padding: 24px 28px 32px;
  color: var(--rss-color-text);
}
.settings-topline {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}
.computer-return {
  display: flex;
  align-items: center;
  gap: 10px;
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
  color: inherit;
  padding: 0;
  min-width: 0;
}
.computer-thumbnail {
  background: url("../home/model-a.png") center / contain no-repeat;
  flex-shrink: 0;
  width: 66px;
  height: 46px;
}
.computer-return strong {
  display: block;
  max-width: 240px;
  overflow-wrap: anywhere;
  font-size: 13px;
}
.computer-return small {
  display: block;
  margin-top: 5px;
  font-size: 11px;
  color: var(--rss-color-text-muted);
}
.preview-label {
  color: var(--rss-color-text-muted);
  background: var(--rss-color-surface);
  padding: 6px 10px;
  border: 1px solid var(--rss-color-border);
  border-radius: 6px;
  font-size: 11px;
}
.settings-layout {
  display: grid;
  grid-template-columns: minmax(240px, 1fr) minmax(280px, 1.2fr);
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  overflow: hidden;
  background: var(--rss-color-bg);
}
.settings-directory {
  min-width: 0;
  padding: 22px 18px;
}
.directory-heading {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 17px;
}
.directory-heading h2 {
  font-size: 20px;
  margin: 0;
}
.directory-heading > span {
  font-size: 11px;
  color: var(--rss-color-text-muted);
}
.settings-list {
  display: grid;
  gap: 8px;
  margin-top: 18px;
}
.settings-list button,
.settings-featured button {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 14px 12px;
  width: 100%;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-bg);
  text-align: left;
  cursor: pointer;
  color: inherit;
}
.settings-list button:hover,
.settings-featured button:hover {
  background: var(--rss-color-surface);
}
.settings-list button[aria-current] {
  background: color-mix(
    in srgb,
    var(--rss-color-accent, #367be6) 12%,
    var(--rss-color-bg)
  );
  border-color: var(--rss-color-accent, #367be6);
}
.entry-copy,
.settings-featured button > span:nth-child(2) {
  flex: 1;
  min-width: 0;
}
.settings-list strong,
.settings-featured strong {
  display: block;
  font-size: 13px;
  font-weight: 600;
}
.settings-list small,
.settings-featured small {
  display: block;
  margin-top: 4px;
  font-size: 11px;
  color: var(--rss-color-text-muted);
  line-height: 1.5;
}
.setting-icon {
  width: 33px;
  height: 33px;
  flex-shrink: 0;
  display: grid;
  place-items: center;
  border-radius: 9px;
  color: white;
  background: #587bd8;
}
.purple {
  background: #8c72cc;
}
.pink {
  background: #c46187;
}
.green {
  background: #378765;
}
.cyan {
  background: #338797;
}
.orange {
  background: #ae723a;
}
.indigo {
  background: #6769b8;
}
.gray {
  background: #76808e;
}
.settings-detail {
  min-width: 0;
  padding: 28px;
  border-left: 1px solid var(--rss-color-border);
  background: color-mix(
    in srgb,
    var(--rss-color-surface) 55%,
    var(--rss-color-bg)
  );
}
.settings-welcome {
  text-align: center;
  padding: 24px 0 28px;
}
.welcome-icon {
  display: grid;
  place-items: center;
  width: 76px;
  height: 76px;
  margin: 0 auto 20px;
  border-radius: 23px;
  color: #687eb8;
  background: color-mix(in srgb, #8aa5ec 16%, var(--rss-color-bg));
}
.settings-welcome h2 {
  font-size: 23px;
  letter-spacing: -0.5px;
  margin: 0 0 12px;
}
.settings-welcome p,
.overview-note,
.setting-note {
  color: var(--rss-color-text-muted);
  font-size: 12px;
  line-height: 1.8;
}
.settings-featured {
  display: grid;
  gap: 8px;
}
.settings-featured h3 {
  font-size: 12px;
  margin: 6px 0;
  color: var(--rss-color-text-muted);
}
.overview-note {
  margin: 23px 0 0;
}
.settings-back {
  background: none;
  border: 0;
  padding: 0;
  color: var(--rss-color-text-muted);
  cursor: pointer;
  font-size: 12px;
}
.detail-path {
  color: var(--rss-color-text-muted);
  font-size: 11px;
  margin: 28px 0 18px;
}
.detail-title {
  display: flex;
  gap: 13px;
  align-items: center;
}
.large {
  width: 48px;
  height: 48px;
  border-radius: 13px;
}
.detail-title h2 {
  font-size: 24px;
  margin: 0;
}
.detail-description {
  font-size: 12px;
  color: var(--rss-color-text-muted);
  margin: 18px 0 24px;
}
.setting-rows {
  display: grid;
  gap: 8px;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 15px;
  padding: 19px 16px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-bg);
  font-size: 12px;
}
.setting-row strong {
  font-weight: 500;
}
.setting-row > span {
  white-space: nowrap;
  color: var(--rss-color-text-muted);
}
.connection-note {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  margin-top: 32px;
  border-top: 1px solid var(--rss-color-border);
  padding-top: 20px;
  color: var(--rss-color-text-muted);
  font-size: 12px;
}
.connection-note svg {
  flex-shrink: 0;
}
.connection-note p {
  line-height: 1.8;
  margin: 6px 0;
}
.empty-search {
  font-size: 12px;
  line-height: 1.7;
  color: var(--rss-color-text-muted);
}
.settings-mac .settings-layout {
  grid-template-columns: 216px minmax(0, 1fr);
}
.settings-mac .settings-directory {
  background: var(--rss-color-surface);
  padding: 20px 12px;
}
.settings-mac .directory-heading {
  padding: 0 6px;
}
.settings-mac .directory-heading h2 {
  font-size: 16px;
}
.settings-mac .settings-list {
  gap: 3px;
}
.settings-mac .settings-list button {
  background: transparent;
  border-color: transparent;
  padding: 8px;
}
.settings-mac .settings-list button[aria-current] {
  background: var(--rss-color-accent, #367be6);
  color: white;
}
.settings-mac .settings-list small {
  display: none;
}
.settings-mac .settings-list .setting-icon {
  width: 27px;
  height: 27px;
  border-radius: 7px;
}
.settings-mac .settings-detail {
  padding: 30px;
}
.settings-mac .setting-rows {
  gap: 0;
  border: 1px solid var(--rss-color-border);
  border-radius: 10px;
  background: var(--rss-color-bg);
  padding: 0 14px;
}
.settings-mac .setting-row {
  border: 0;
  border-radius: 0;
  border-bottom: 1px solid var(--rss-color-border);
  padding: 17px 0;
}
.settings-mac .setting-row:last-child {
  border-bottom: 0;
}
button:focus-visible,
h2:focus-visible {
  outline: 2px solid var(--rss-color-accent, #367be6);
  outline-offset: 3px;
}
@container system-settings (max-width: 620px) {
  .settings-layout,
  .settings-mac .settings-layout {
    grid-template-columns: 1fr;
  }
  .settings-detail,
  .has-detail .settings-directory {
    display: none;
  }
  .has-detail .settings-detail {
    display: block;
    border-left: 0;
  }
  .settings-mac .settings-list small {
    display: block;
  }
}
@media (max-width: 900px) {
  .system-settings {
    padding: 18px;
  }
  .settings-layout,
  .settings-mac .settings-layout {
    grid-template-columns: 1fr;
  }
  .settings-detail {
    display: none;
  }
  .has-detail .settings-directory {
    display: none;
  }
  .has-detail .settings-detail {
    display: block;
    border-left: 0;
  }
  .settings-mac .settings-list small {
    display: block;
  }
}
@media (max-width: 500px) {
  .system-settings {
    padding: 12px;
  }
  .settings-topline {
    align-items: flex-start;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 16px;
  }
  .settings-detail,
  .settings-mac .settings-detail {
    padding: 20px 16px;
  }
  .computer-return strong {
    max-width: 220px;
  }
}
@media (forced-colors: active) {
  .setting-icon {
    border: 1px solid ButtonText;
  }
  .settings-list button[aria-current] {
    outline: 2px solid Highlight;
  }
}
</style>
