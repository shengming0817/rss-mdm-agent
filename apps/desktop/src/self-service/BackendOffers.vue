<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { Controller } from "./controller";
import type { BackendTask } from "./types";
import { Package, Terminal, Sparkles } from "@rss-mdm-agent/ui";
import { resourceKey } from "../assistant/resource-context";
const props = defineProps<{
  controller: Controller;
  contextKey?: string;
  kind: "software" | "script";
}>();
const emit = defineEmits<{
  askAi: [item: BackendTask, trigger: HTMLElement];
}>();
const c = props.controller;
const s = c.state;
const selectedKey = ref("");
const resourceHeading = ref<HTMLElement>();
let resourceTrigger: HTMLElement | undefined;
async function openResource(item: BackendTask, trigger: HTMLElement) {
  resourceTrigger = trigger;
  selectedKey.value = resourceKey(item);
  await nextTick();
  resourceHeading.value?.focus({ preventScroll: true });
  resourceHeading.value?.scrollIntoView({ block: "start" });
}
async function closeResource() {
  selectedKey.value = "";
  await nextTick();
  if (resourceTrigger?.isConnected) resourceTrigger.focus();
}
const resource = computed(() =>
  s.snapshot?.available.find((item) => resourceKey(item) === selectedKey.value),
);
const groups = computed(() => [
  {
    kind: props.kind,
    label: props.kind === "software" ? "软件" : "工具",
    items:
      s.snapshot?.available.filter(
        (item) => item.presentation.summary.kind === props.kind,
      ) ?? [],
  },
]);
function operationLabel(value: string) {
  switch (value) {
    case "install":
      return "安装";
    case "uninstall":
      return "卸载";
    case "detect":
      return "检测";
    default:
      return "未知操作";
  }
}
</script>
<template>
  <section class="backend-offers" aria-label="当前已签名任务">
    <section
      v-for="group in groups"
      :key="group.kind"
      class="resource-group"
      :aria-label="group.label"
    >
      <div class="section-heading">
        <h3>{{ group.label }}</h3>
        <span>{{ group.items.length }} 项 · 当前加载</span>
      </div>
      <p v-if="!group.items.length">当前加载目录没有此类资源。</p>
      <div class="catalog-grid">
        <article
          v-for="offer in group.items"
          class="resource-card"
          :class="{
            selected: resourceKey(offer) === (contextKey ?? selectedKey),
          }"
          :key="resourceKey(offer)"
        >
          <button
            type="button"
            data-action="resource-details"
            class="resource-title"
            :aria-pressed="resourceKey(offer) === selectedKey"
            @click="openResource(offer, $event.currentTarget as HTMLElement)"
          >
            <span class="resource-icon"
              ><Package
                v-if="offer.presentation.summary.kind === 'software'"
                :size="22"
                aria-hidden="true" /><Terminal
                v-else
                :size="22"
                aria-hidden="true"
            /></span>
            <span
              ><strong>{{ offer.presentation.title }}</strong
              ><small>{{
                offer.presentation.summary.kind === "software"
                  ? "软件资源"
                  : "后台固定脚本"
              }}</small></span
            >
          </button>
          <p>分类与说明未提供</p>
          <span class="status-badge">{{
            false ? "S1 样本" : false ? "预览" : "执行服务目录"
          }}</span>
          <button
            type="button"
            class="resource-link"
            @click="openResource(offer, $event.currentTarget as HTMLElement)"
          >
            查看资源信息
          </button>
        </article>
      </div>
    </section>
    <section v-if="resource" class="resource-details" aria-label="资源详情">
      <div class="section-heading">
        <h3 ref="resourceHeading" tabindex="-1">
          {{ resource.presentation.title }}
        </h3>
        <button type="button" @click="closeResource">关闭详情</button>
      </div>
      <p>
        {{
          resource.presentation.summary.kind === "software"
            ? "软件详情"
            : resource.presentation.summary.kind === "collection"
              ? "采集详情"
              : "脚本详情"
        }}
        · 未提供分类 · 后台未提供资源说明
      </p>
      <ol v-if="resource.presentation.summary.kind === 'software'">
        <li
          v-for="(step, index) in resource.presentation.summary.steps"
          :key="index"
        >
          {{ step.package }} · {{ step.version }} ·
          {{ step.identity === "system" ? "系统账号" : "当前用户" }}
          <button
            :disabled="s.busy || s.uncertain"
            @click="c.observe(resource, index)"
          >
            检测当前状态
          </button>
        </li>
      </ol>
      <p v-else>
        运行身份：{{
          resource.presentation.summary.kind === "collection" ||
          (resource.presentation.summary.kind === "script" &&
            resource.presentation.summary.identity === "system")
            ? "系统账号"
            : "当前用户"
        }}
      </p>
      <p>有效期：{{ new Date(resource.expiresAt * 1000).toLocaleString() }}</p>
      <p>
        列表可见不代表已安装或获准执行；后台未提供完整能力判定，实际操作由执行服务裁决。
      </p>
      <div class="resource-actions">
        <button
          type="button"
          data-action="ask-ai"
          @click="emit('askAi', resource, $event.currentTarget as HTMLElement)"
        >
          <Sparkles :size="16" aria-hidden="true" />询问 AI
        </button>
        <button
          v-if="resource.userInitiated"
          class="primary-button"
          :disabled="s.busy || s.uncertain"
          @click="c.select(resource)"
        >
          确认此操作
        </button>
        <span v-else>由后台派发，执行服务处理。</span>
      </div>
    </section>
    <section v-if="s.item" aria-label="操作确认">
      <h2>确认执行 {{ s.item.presentation.title }}</h2>
      <template v-if="s.item.presentation.summary.kind === 'software'">
        <p>操作：{{ operationLabel(s.item.presentation.summary.intent) }}</p>
        <ol>
          <li
            v-for="(step, index) in s.item.presentation.summary.steps"
            :key="index"
          >
            {{ step.package }} {{ step.version }} ·
            {{ step.identity === "system" ? "系统账号" : "当前用户" }}
          </li>
        </ol>
      </template>
      <p v-else>
        {{
          s.item.presentation.summary.kind === "collection"
            ? "原生设备采集"
            : "后台固定脚本"
        }}
        ·
        {{
          s.item.presentation.summary.kind === "collection" ||
          (s.item.presentation.summary.kind === "script" &&
            s.item.presentation.summary.identity === "system")
            ? "系统账号"
            : "当前用户"
        }}
      </p>
      <button :disabled="s.busy || s.uncertain" @click="c.confirm">
        确认并执行
      </button>
      <button :disabled="s.busy" @click="s.item = null">返回</button>
    </section>
  </section>
</template>
