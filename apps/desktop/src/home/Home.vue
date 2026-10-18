<script setup lang="ts">
import { computed, ref } from "vue";
import HomeAssistant from "./HomeAssistant.vue";
import {
  Monitor,
  UserRound,
  UiPopover,
  Package,
  Terminal,
  Settings,
  Activity,
  ChevronRight,
} from "@rss-mdm-agent/ui";
import type { Controller } from "../self-service/controller";
import type { DeviceOverview } from "../self-service/types";
const props = defineProps<{
  controller: Controller;
  preview: boolean;
  draft: string;
  connected: boolean;
  device?: DeviceOverview | null;
}>();
const emit = defineEmits<{
  navigate: [page: string];
  "update:draft": [text: string];
}>();
const software = computed(
  () =>
    props.controller.state.snapshot?.available.filter(
      (item) => item.summary.kind === "software",
    ) ?? [],
);
const addressesOpen = ref(false);
const shortcuts = [
  {
    page: "system-settings",
    title: "系统设置",
    description: "网络、显示与电源",
  },
  {
    page: "software",
    title: "软件管理",
    description: "浏览可用软件",
  },
  {
    page: "tools",
    title: "工具",
    description: "查看组织提供的工具",
  },
];
</script>
<template>
  <div class="computer-home">
    <div class="home-main">
      <section class="device-card" aria-label="我的电脑概览">
        <div class="computer-overview">
          <div class="computer-model">
            <div
              class="computer-chassis"
              role="img"
              aria-label="银色笔记本电脑示意图"
            />
            <div class="computer-screen">
              <div v-if="software.length" class="screen-apps">
                <button
                  v-for="(item, index) in software.slice(0, 4)"
                  :key="item.task"
                  type="button"
                  :title="`在软件中心查看 ${item.title}`"
                  @click="emit('navigate', 'software')"
                >
                  <span class="app-tile" :class="`tile-${index}`"
                    ><Package :size="22" aria-hidden="true" /></span
                  ><span>{{ item.title }}</span>
                </button>
              </div>
              <div v-else class="screen-welcome">
                <Monitor :size="30" aria-hidden="true" /><strong
                  >你的工作，从这里开始</strong
                ><span>软件与工具，触手可及</span>
              </div>
            </div>
          </div>
          <div class="computer-identity" aria-label="本机信息">
            <span
              class="identity-name"
              :aria-label="`计算机名：${device?.computerName ?? '未读取'}`"
              :title="device?.computerName ?? '计算机名未读取'"
            >
              <Monitor :size="17" aria-hidden="true" /><span>{{
                device?.computerName ?? "—"
              }}</span>
            </span>
            <span class="identity-divider" aria-hidden="true" />
            <span
              :aria-label="`IP 地址：${device?.ipAddresses?.join('、') || (device?.ipAddresses ? '无可用地址' : '未读取')}`"
              :title="
                device?.ipAddresses?.join('、') ||
                (device?.ipAddresses ? '无可用地址' : 'IP 地址未读取')
              "
            >
              <Activity :size="14" aria-hidden="true" />
              <span>{{ device?.ipAddresses?.[0] ?? "—" }}</span>
              <UiPopover
                v-if="device?.ipAddresses && device.ipAddresses.length > 1"
                v-model:open="addressesOpen"
                label="全部 IP 地址"
              >
                <template #trigger
                  ><button
                    class="ip-more"
                    type="button"
                    aria-label="查看所有 IP 地址"
                  >
                    +{{ device.ipAddresses.length - 1 }}
                  </button></template
                >
                <ul class="ip-list">
                  <li v-for="address in device.ipAddresses" :key="address">
                    {{ address }}
                  </li>
                </ul>
              </UiPopover>
            </span>
            <span
              :aria-label="`系统登录用户：${device?.loginUser ?? '未读取'}`"
              :title="device?.loginUser ?? '系统登录用户未读取'"
            >
              <UserRound :size="14" aria-hidden="true" /><span>{{
                device?.loginUser ?? "—"
              }}</span>
            </span>
          </div>
        </div>
        <div class="model-toolbar">
          <span
            >{{ software.length }} 项软件资源 ·
            {{
              preview
                ? "样本目录 · 不代表本机已安装软件"
                : "可用目录 · 不代表本机已安装软件"
            }}</span
          ><button type="button" @click="emit('navigate', 'software')">
            管理软件 <ChevronRight :size="13" aria-hidden="true" />
          </button>
        </div>
      </section>
      <section class="home-notice" aria-label="设备状态">
        <span class="notice-icon"
          ><Activity :size="21" aria-hidden="true"
        /></span>
        <div>
          <strong>{{
            controller.state.error ? "任务需要关注" : "每一次操作，都有记录"
          }}</strong>
          <p>
            {{ controller.state.error || "查看请求进度、确认事项和执行结果。" }}
          </p>
        </div>
        <button type="button" @click="emit('navigate', 'tasks')">
          查看任务
        </button>
      </section>
      <nav class="home-shortcuts" aria-label="电脑快捷入口">
        <button
          v-for="item in shortcuts"
          :key="item.page"
          type="button"
          @click="emit('navigate', item.page)"
        >
          <span class="shortcut-icon"
            ><Settings
              v-if="item.page === 'system-settings'"
              :size="20"
              aria-hidden="true" /><Package
              v-else-if="item.page === 'software'"
              :size="20"
              aria-hidden="true" /><Terminal
              v-else
              :size="20"
              aria-hidden="true" /></span
          ><span
            ><strong>{{ item.title }}</strong
            ><small>{{ item.description }}</small></span
          ><ChevronRight :size="14" aria-hidden="true" />
        </button>
      </nav>
    </div>
    <HomeAssistant
      :draft="draft"
      :connected="connected"
      @update:draft="emit('update:draft', $event)"
      @navigate="emit('navigate', $event)"
    />
  </div>
</template>
<style scoped>
.computer-home {
  --home-bg: var(--rss-color-bg);
  --home-muted: var(--rss-color-text-muted);
  color: var(--rss-color-text);
  display: grid;
  grid-template-columns: minmax(0, 1fr) 246px;
  min-height: 100%;
  background: var(--rss-color-bg);
}
.home-main {
  min-width: 0;
  padding: 24px;
  background: color-mix(in srgb, var(--home-bg) 94%, #b7b9d9);
}
.model-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.device-card {
  padding: 20px 20px 12px;
}
.computer-overview {
  width: min(720px, 100%);
  margin: 0 auto;
}
.computer-identity {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 24px;
  margin: 10px auto 24px;
  color: var(--home-muted);
  font-size: 12px;
  line-height: 1.4;
}
.computer-identity > span:not(.identity-divider) {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 42px;
  max-width: 100%;
  overflow-wrap: anywhere;
}
.computer-identity svg {
  flex: none;
  opacity: 0.65;
}
.computer-identity .identity-name {
  color: var(--rss-color-text);
  font-size: 15px;
  font-weight: 550;
}
.identity-divider {
  height: 13px;
  width: 1px;
  background: var(--rss-color-border);
}

.ip-more {
  border: 0;
  padding: 2px 5px;
  border-radius: 4px;
  color: var(--rss-color-accent);
  background: var(--rss-color-accent-bg);
  font-size: 10px;
}
.ip-list {
  list-style: none;
  margin: 0;
  padding: 4px;
  font-size: 12px;
  line-height: 1.8;
}
.computer-model {
  position: relative;
  width: 100%;
  aspect-ratio: 1508/780;
  container-type: inline-size;
  margin: 0 auto;
}
.computer-chassis {
  position: absolute;
  inset: 0;
  /* Crop only the source's empty top/bottom margins; keep its image ratio. */
  background: url("./model-a.png") center / 100% auto no-repeat;
  mix-blend-mode: darken;
}
.computer-screen {
  position: absolute;
  left: 17.25%;
  /* Original 1508 × 1043 image, centered in the 1508 × 780 viewport. */
  top: 8.013%;
  width: 65.45%;
  height: 79.162%;
  overflow: auto;
  background:
    radial-gradient(ellipse at 110% 20%, #f9d2c5, transparent 70%),
    radial-gradient(ellipse at 45% 110%, #d8c5ef, transparent 65%),
    linear-gradient(135deg, #83a8dd, #c3b7e4 60%, #edc8d7);
}
.screen-apps {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  padding: 3cqw 2cqw;
  gap: 1cqw;
}
.screen-apps button {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.75cqw;
  border: 1px solid transparent;
  border-radius: 1.5cqw;
  padding: 1.75cqw 0.75cqw;
  background: transparent;
  color: #243347;
  font-size: 2.25cqw;
  min-width: 0;
}
.screen-apps button:hover {
  background: #ffffff40;
  border-color: #ffffff80;
}
.screen-apps button > span:last-child {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.app-tile {
  display: grid;
  place-items: center;
  width: 8.5cqw;
  height: 8.5cqw;
  border-radius: 2.25cqw;
  color: white;
  background: linear-gradient(145deg, #b296ea, #7052b9);
  box-shadow: 0 4px 9px #45537025;
}
.app-tile svg {
  width: 5.5cqw;
  height: 5.5cqw;
}
.tile-1 {
  background: linear-gradient(145deg, #efcc88, #d69a43);
}
.tile-2 {
  background: linear-gradient(145deg, #8fd3c5, #47968c);
}
.tile-3 {
  background: linear-gradient(145deg, #8cbbef, #4d80c7);
}
.screen-welcome {
  display: flex;
  height: 100%;
  flex-direction: column;
  gap: 2.5cqw;
  align-items: center;
  justify-content: center;
  color: #35415a;
  font-size: 3cqw;
}
.screen-welcome span {
  font-size: 2.5cqw;
}
.screen-welcome > svg {
  width: 7.5cqw;
  height: 7.5cqw;
}
.model-toolbar {
  margin: 4px 0 14px;
  font-size: 10px;
  color: var(--home-muted);
}
.model-toolbar button,
.home-notice > button {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  border: 1px solid var(--rss-color-border);
  border-radius: 6px;
  background: var(--home-bg);
  color: var(--rss-color-text);
  padding: 6px 9px;
  font-size: 10px;
}
.home-notice {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 13px 0;
  padding: 15px;
  background: color-mix(in srgb, var(--home-bg) 94%, #e5c77e);
  border: 1px solid var(--rss-color-border);
  border-radius: 11px;
}
.notice-icon {
  display: grid;
  place-items: center;
  background: #d4b45b18;
  color: #a48b48;
  border-radius: 8px;
  width: 33px;
  height: 33px;
  flex: none;
}
.home-notice > div {
  flex: 1;
  min-width: 0;
}
.home-notice strong {
  font-size: 11px;
}
.home-notice p {
  font-size: 10px;
  line-height: 1.6;
  margin: 5px 0 0;
  color: var(--home-muted);
}
.home-shortcuts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}
.home-shortcuts button {
  display: flex;
  align-items: center;
  gap: 10px;
  text-align: left;
  padding: 16px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 10px;
  background: var(--home-bg);
  color: var(--rss-color-text);
}
.shortcut-icon {
  color: #809dbc;
  background: #849dc211;
  padding: 6px;
  border-radius: 8px;
  display: flex;
}
.home-shortcuts strong {
  display: block;
  font-size: 12px;
  font-weight: 550;
}
.home-shortcuts small {
  display: block;
  font-size: 9px;
  margin-top: 5px;
  color: var(--home-muted);
}
.home-shortcuts button > svg {
  margin-left: auto;
  flex: none;
  color: var(--home-muted);
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
button {
  cursor: pointer;
  font-family: inherit;
}
button:focus-visible,
textarea:focus-visible {
  outline: 2px solid #6d95c8;
  outline-offset: 3px;
}
button:not(:disabled):hover {
  filter: brightness(0.97);
}
@media (prefers-color-scheme: dark) {
  .computer-chassis {
    mix-blend-mode: normal;
    border-radius: 10px;
  }
}
@media (max-width: 1250px) {
  .computer-home {
    grid-template-columns: minmax(0, 1fr) 220px;
  }
  .home-main {
    padding: 20px 16px;
  }
  .home-shortcuts button {
    gap: 6px;
    padding: 13px 8px;
  }
  .home-shortcuts button > svg {
    display: none;
  }
  .shortcut-icon {
    padding: 4px;
  }
  .device-card {
    padding: 17px 14px 12px;
  }
}
@media (max-width: 1050px) {
  .computer-home {
    grid-template-columns: minmax(0, 1fr);
  }
  .home-shortcuts small {
    line-height: 1.5;
  }
}
@media (max-width: 500px) {
  .home-main {
    padding: 18px 12px;
  }
  .home-shortcuts {
    grid-template-columns: 1fr;
  }
  .home-shortcuts button {
    padding: 13px;
  }
  .home-shortcuts button > svg {
    display: block;
  }
  .home-shortcuts small {
    font-size: 10px;
  }
  .model-toolbar {
    flex-wrap: wrap;
  }
  .home-notice {
    padding: 12px 10px;
    gap: 8px;
  }
}
</style>
