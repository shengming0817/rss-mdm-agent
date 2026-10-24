<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { softwareSummaryKey } from "../software/controller";
import HomeAssistant from "./HomeAssistant.vue";
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";
import { systemSettingsKey } from "../system-settings/controller";
import { homeFacts } from "../system-settings/facts";
const settings = inject(systemSettingsKey, undefined);
const facts = computed(() => homeFacts(settings));
import { UiPopover, Package, Sparkles } from "@rss-mdm-agent/ui";
import type { Controller } from "../self-service/controller";
import type { DeviceOverview } from "../self-service/types";
import { activityKey, taskPhase } from "../self-service/activity";
const props = defineProps<{
  controller: Controller;
  draft: string;
  connected: boolean;
  device?: DeviceOverview | null;
}>();
const emit = defineEmits<{
  navigate: [page: string, section?: string];
  "update:draft": [text: string];
  open: [target: import("../workspace/page-navigation").NavigationTarget];
  task: [request: string];
}>();
const activity = inject(activityKey, undefined);
const summary = inject(softwareSummaryKey, undefined);
const installed = computed(
  () =>
    summary?.value.items.map((item) => ({
      id: item.reference.installation,
      name: item.name,
      letter: item.name.slice(0, 1),
      revision: JSON.stringify(item.reference),
    })) ?? [],
);
const updates = computed(() => summary?.value.updates);
const softwareRead = computed(() => summary?.value.read ?? false);
const addressesOpen = ref(false);
</script>
<template>
  <div class="computer-home design-page">
    <div class="home-feature">
      <section class="home-hero" aria-label="我的电脑概览">
        <div class="home-hero-copy">
          <span class="design-eyebrow">● YOUR EVERYDAY COMPANION</span>
          <h2>我的电脑，<br />状态与操作一目了然。</h2>
          <p>
            {{ device?.attributes.computerName ?? "你的电脑" }} · 设备概览<br />常用应用、设备状态，都在这里。
          </p>
          <button
            class="design-button"
            type="button"
            @click="emit('navigate', 'system-settings', 'about')"
          >
            了解这台电脑 →
          </button>
        </div>
        <div class="home-laptop" aria-hidden="true">
          <div class="laptop-screen">
            <small>你的每一天，都有好工具相伴</small
            ><span class="laptop-clock">09:41</span>
            <div class="laptop-dock"><i>W</i><i>N</i><i>◎</i><i>↻</i></div>
          </div>
          <div class="laptop-base" />
        </div>
      </section>
      <HomeAssistant
        presentation="hero"
        :draft="draft"
        :connected="connected"
        @update:draft="emit('update:draft', $event)"
        @navigate="emit('navigate', $event)"
      />
    </div>
    <div class="home-stats">
      <button
        type="button"
        @click="emit('navigate', 'system-settings', 'network')"
      >
        <span class="stat-label"
          ><SettingsGlyph kind="network" :size="17" />网络连接</span
        ><strong>{{ facts.network }}</strong
        ><small>{{
          settings?.state.busy.has("network")
            ? "读取中…"
            : settings?.state.errors.get("network") || facts.networkDetail
        }}</small>
      </button>
      <button
        type="button"
        @click="emit('navigate', 'system-settings', 'storage')"
      >
        <span class="stat-label"
          ><SettingsGlyph kind="storage" :size="17" />存储空间</span
        ><strong
          >{{ facts.storageFree
          }}<em v-if="facts.storageUsed !== undefined">GB 可用</em></strong
        ><small>{{ facts.storageDetail }}</small
        ><span
          v-if="facts.storageUsed !== undefined"
          class="stat-track storage-track"
          ><i :style="{ width: `${facts.storageUsed}%` }"
        /></span>
      </button>
      <button
        type="button"
        @click="emit('navigate', 'system-settings', 'power')"
      >
        <span class="stat-label"
          ><SettingsGlyph kind="power" :size="17" />电池与电源</span
        ><strong>{{ facts.battery }}</strong
        ><small>{{ facts.batteryDetail }}</small
        ><span
          v-if="facts.batteryPercent !== undefined"
          class="stat-track power-track"
          ><i
            :style="{
              width: `${facts.batteryPercent}%`,
            }"
        /></span>
      </button>
      <button
        type="button"
        @click="emit('navigate', 'software-management', 'installed')"
      >
        <span class="stat-label"
          ><Package :size="17" aria-hidden="true" />我的应用</span
        ><strong
          >{{
            softwareRead
              ? summary?.complete
                ? installed.length
                : `已加载 ${installed.length}`
              : "—"
          }}<em>{{ summary?.complete ? "个已安装" : "项安装观察" }}</em></strong
        ><small>{{
          softwareRead
            ? `${summary?.complete ? "完整清单" : "部分或过期清单"} · 当前加载 ${installed.length} 项 · ${updates ?? "—"} 个固定更新候选`
            : "软件清单尚未读取"
        }}</small
        ><small v-if="summary?.loading">读取中…</small
        ><small v-if="summary?.error" role="status">{{ summary.error }}</small
        ><small v-if="summary?.collectedAt"
          >观察于
          {{ new Date(summary.collectedAt * 1000).toLocaleString() }}</small
        ><span class="stat-link">查看更新 →</span>
      </button>
    </div>
    <div class="home-bottom">
      <section>
        <div class="section-heading">
          <h3>已安装应用</h3>
          <button
            class="text-link"
            @click="emit('navigate', 'software-management')"
          >
            发现更多 ›
          </button>
        </div>
        <div class="home-apps">
          <button
            v-for="item in installed.slice(0, 4)"
            :key="item.id"
            @click="
              emit('open', {
                kind: 'resource',
                page: 'software-management',
                id: item.id,
                revision: item.revision,
              })
            "
          >
            <span class="home-app-icon" :class="item.id">{{ item.letter }}</span
            ><span>{{ item.name }}</span>
          </button>
          <p v-if="!installed.length" class="home-empty">
            {{ softwareRead ? "当前加载范围没有应用。" : "软件清单尚未读取。" }}
          </p>
        </div>
        <div class="home-tip">
          <Sparkles :size="17" aria-hidden="true" /><span
            >小提示：使用 ⌘ / Ctrl K，快速找到你需要的应用和工具。</span
          >
        </div>
      </section>
      <section>
        <div class="section-heading">
          <h3>最近动态</h3>
          <button class="text-link" @click="emit('navigate', 'tasks')">
            查看任务记录 ›
          </button>
        </div>
        <div class="home-activity">
          <button
            v-for="item in activity?.state.items ?? []"
            :key="item.details.action.requestId"
            @click="emit('task', item.details.action.requestId)"
            class="activity-row"
          >
            <span
              ><strong
                >{{ item.details.action.operation.action }} ·
                {{ item.details.action.operation.resource.id }}</strong
              >
              <small
                >{{ taskPhase[item.details.status.phase] ?? "状态未知"
                }}{{
                  item.details.status.cancelRequested
                    ? " · 已请求取消，终止单独核实"
                    : ""
                }}</small
              ></span
            >
            <time>{{ new Date(item.recordedAtUnixMs).toLocaleString() }}</time>
          </button>
          <p v-if="!activity?.state.loaded" class="home-empty">
            {{ activity?.available ? "动态尚未读取" : "执行服务未连接" }}
          </p>
          <p v-else-if="!activity.state.items.length" class="home-empty">
            {{
              activity.state.next
                ? "当前页无可见记录，仍可继续读取"
                : "暂无操作记录"
            }}
          </p>
          <p v-if="activity?.state.error" role="status">
            {{ activity.state.error }}
          </p>
          <button
            v-if="activity?.state.next"
            :disabled="activity.state.loading"
            @click="activity.load(true)"
          >
            继续读取动态
          </button>
          <div v-if="controller.state.error" role="status">
            <span
              ><strong>任务需要关注</strong
              ><small>{{ controller.state.error }}</small></span
            ><button class="text-link" @click="emit('navigate', 'tasks')">
              查看处理详情
            </button>
          </div>
        </div>
      </section>
    </div>
    <div class="computer-identity" aria-label="本机信息">
      <span
        class="identity-name"
        :aria-label="`计算机名：${device?.attributes.computerName ?? '未读取'}`"
        >{{ device?.attributes.computerName ?? "计算机名未读取" }}</span
      ><span
        :title="device?.attributes.ipAddresses?.join('、') || 'IP 地址未读取'"
        >{{ device?.attributes.ipAddresses?.[0] ?? "IP 地址未读取"
        }}<UiPopover
          v-if="
            device?.attributes.ipAddresses &&
            device.attributes.ipAddresses.length > 1
          "
          v-model:open="addressesOpen"
          label="全部 IP 地址"
          ><template #trigger
            ><button aria-label="查看所有 IP 地址" class="ip-more">
              +{{ device.attributes.ipAddresses.length - 1 }}
            </button></template
          >
          <ul>
            <li v-for="address in device.attributes.ipAddresses" :key="address">
              {{ address }}
            </li>
          </ul></UiPopover
        ></span
      ><span>{{ device?.attributes.loginUser ?? "系统登录用户未读取" }}</span>
    </div>
  </div>
</template>
<style scoped>
.computer-home {
  color: var(--rss-color-text);
}
.home-feature {
  display: grid;
  grid-template-columns: 1.7fr 1fr;
  gap: 18px;
}
.home-hero {
  position: relative;
  overflow: hidden;
  min-height: 326px;
  border-radius: 18px;
  padding: 30px;
  background: linear-gradient(115deg, #e8ede7, #f1eee8);
  color: #354d43;
}
.home-hero::after {
  content: "";
  position: absolute;
  width: 470px;
  height: 470px;
  right: -150px;
  top: -220px;
  border-radius: 50%;
  border: 1px solid #748e6b0c;
  background: #ffffff15;
}
.home-hero-copy {
  position: relative;
  z-index: 1;
}
.home-hero .design-eyebrow {
  font-size: 8px;
  letter-spacing: 1.8px;
  color: #7f9e91;
  margin: 0 0 20px;
}
.home-hero h2 {
  font-size: 30px;
  line-height: 1.4;
  margin: 0 0 15px;
  letter-spacing: -0.9px;
}
.home-hero p {
  font-size: 11px;
  line-height: 1.9;
  color: #8c9e94;
  margin: 0 0 22px;
}
.home-laptop {
  position: absolute;
  right: 4%;
  bottom: 30px;
  width: 48%;
  transform: rotate(-7deg);
}
.laptop-screen {
  aspect-ratio: 1.55;
  border: 5px solid #3d4a4f;
  border-radius: 10px 10px 3px 3px;
  background: linear-gradient(145deg, #8aa2b3, #e8cfb0);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 20px 10px 10px;
  position: relative;
}
.laptop-screen::before {
  content: "";
  position: absolute;
  width: 20%;
  height: 9px;
  background: #3d4a4f;
  top: -1px;
  border-radius: 0 0 4px 4px;
}
.laptop-screen small {
  font-size: 7px;
  color: #ffffff9c;
}
.laptop-clock {
  font-size: 34px;
  font-weight: 300;
  color: #fff;
  margin-top: 6px;
}
.laptop-dock {
  display: flex;
  gap: 5px;
  padding: 6px;
  margin-top: auto;
  border: 1px solid #ffffff40;
  border-radius: 10px;
  background: #ffffff20;
}
.laptop-dock i {
  display: grid;
  place-items: center;
  width: 27px;
  height: 27px;
  background: #557c66;
  border-radius: 7px;
  font-style: normal;
  color: white;
  font-size: 19px;
}
.laptop-dock i:nth-child(2) {
  background: #e6c572;
  color: #947633;
}
.laptop-dock i:nth-child(3) {
  background: #8ab2b6;
}
.laptop-dock i:nth-child(4) {
  background: #7e9dca;
}
.laptop-base {
  height: 5px;
  background: linear-gradient(#c5d0d2, #91a3a9);
  border-radius: 0 0 80% 80%;
  margin: 0 -12px;
  box-shadow: 0 15px 24px #39534712;
}
.home-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
  margin-top: 18px;
}
.home-stats > button {
  text-align: left;
  background: var(--rss-color-surface);
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  padding: 18px;
  min-height: 135px;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  color: var(--rss-color-text);
}
.stat-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 10px;
  color: var(--rss-color-text-muted);
}
.home-stats strong {
  font-size: 23px;
  margin-top: 13px;
  line-height: 1.4;
  font-weight: 650;
}
.home-stats em {
  font-size: 10px;
  font-style: normal;
  color: var(--rss-color-text-muted);
  font-weight: 400;
  margin-left: 8px;
}
.home-stats small {
  font-size: 9px;
  color: var(--rss-color-text-muted);
  margin-top: 5px;
}
.stat-track {
  display: block;
  width: 100%;
  height: 4px;
  border-radius: 4px;
  background: var(--rss-color-neutral-bg);
  margin-top: 12px;
  overflow: hidden;
}
.stat-track i {
  display: block;
  height: 100%;
  background: #91b0a1;
  border-radius: 4px;
}
.storage-track i {
  background: #a4adc5;
}
.power-track i {
  background: #a9b59c;
}
.stat-link {
  font-size: 9px;
  color: var(--rss-color-accent);
  margin-top: 12px;
}
.home-bottom {
  display: grid;
  grid-template-columns: 1.55fr 1fr;
  gap: 23px;
  margin-top: 25px;
}
.computer-home .section-heading {
  margin: 0 0 14px;
}
.computer-home .section-heading h3 {
  font-size: 16px;
  margin: 0;
}
.computer-home .section-heading .text-link {
  font-size: 10px;
  padding: 0;
}
.home-apps {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}
.home-apps button {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 13px;
  padding: 20px 10px;
  background: var(--rss-color-surface);
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  font-size: 10px;
  min-height: 113px;
}
.home-app-icon {
  display: grid;
  place-items: center;
  width: 45px;
  height: 45px;
  border-radius: 12px;
  color: white;
  font-size: 27px;
  font-weight: 650;
  background: linear-gradient(140deg, #799782, #3e6456);
  box-shadow: 0 2px 5px #24372b10;
}
.home-app-icon.notes {
  background: linear-gradient(140deg, #f2db97, #e0b951);
  color: #96732e;
}
.home-app-icon.browser {
  background: linear-gradient(140deg, #b2d3d0, #739ea5);
}
.home-app-icon.sync {
  background: linear-gradient(140deg, #a5bfe1, #6b8fc2);
}
.home-activity {
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  padding: 0 17px;
}
.home-activity > div,
.activity-row {
  width: 100%;
  text-align: left;
  color: inherit;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 56px;
  padding: 13px 0;
  border-bottom: 1px solid var(--rss-color-border);
}
.home-activity > div:last-child {
  border: 0;
}
.activity-symbol {
  width: 30px;
  height: 30px;
  border-radius: 9px;
  background: #edf3ef;
  color: #88a599;
  display: grid;
  place-items: center;
  flex: none;
}
.home-activity strong {
  display: block;
  font-size: 10px;
  font-weight: 550;
}
.home-activity small {
  display: block;
  color: var(--rss-color-text-muted);
  font-size: 9px;
  margin-top: 5px;
}
.home-activity time {
  margin-left: auto;
  font-size: 9px;
  color: var(--rss-color-text-muted);
}
.home-tip {
  display: flex;
  align-items: center;
  gap: 9px;
  border-radius: 10px;
  background: #efede6;
  color: #9a8e73;
  font-size: 9px;
  padding: 11px 13px;
  margin-top: 14px;
}
.computer-identity {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  margin-top: 20px;
  font-size: 9px;
  color: var(--rss-color-text-muted);
}
.ip-more {
  border: 0;
  background: transparent;
  padding: 0 5px;
  font-size: 9px;
}
.home-empty {
  font-size: 11px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
  padding: 20px;
  grid-column: 1 / -1;
}
@container (max-width: 1100px) {
  .home-laptop {
    width: 44%;
    right: 3%;
  }
  .laptop-screen {
    padding-top: 14px;
  }
  .laptop-clock {
    font-size: 27px;
  }
  .home-hero h2 {
    font-size: 27px;
  }
}
@container (max-width: 850px) {
  .home-feature {
    grid-template-columns: 1fr;
  }
  .home-hero {
    min-height: 300px;
  }
  .home-bottom {
    grid-template-columns: 1fr;
  }
  .home-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@container (max-width: 480px) {
  .home-hero {
    padding: 24px;
    min-height: 290px;
  }
  .home-laptop {
    width: 42%;
    right: -2%;
    bottom: 30px;
  }
  .laptop-clock {
    font-size: 20px;
  }
  .laptop-screen small,
  .laptop-dock {
    display: none;
  }
  .home-hero h2 {
    font-size: 27px;
  }
  .home-hero p {
    max-width: 170px;
  }
  .home-stats {
    gap: 10px;
  }
  .home-stats > button {
    padding: 15px;
  }
  .home-apps {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
