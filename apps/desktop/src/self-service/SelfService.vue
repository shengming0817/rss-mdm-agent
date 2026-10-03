<script setup lang="ts">
import { computed, nextTick, ref, onMounted, onUnmounted } from "vue";
import type { Controller } from "./controller";
import type { BackendTask } from "./types";
import TaskDetail from "./TaskDetail.vue";
import { Package, Terminal, Sparkles } from "@rss-mdm-agent/ui";
import { resourceKey } from "../assistant/resource-context";
const props = defineProps<{
  controller: Controller;
  contextKey?: string;
  source: "s1" | "live" | "preview";
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
const groups = computed(() =>
  [
    {
      kind: "software",
      label: "软件",
      items:
        s.snapshot?.available.filter(
          (item) => item.summary.kind === "software",
        ) ?? [],
    },
    {
      kind: "script",
      label: "工具与脚本",
      items:
        s.snapshot?.available.filter(
          (item) => item.summary.kind === "script",
        ) ?? [],
    },
  ].filter(
    (group) =>
      (s.page !== "software" && s.page !== "tools") ||
      group.kind === (s.page === "software" ? "software" : "script"),
  ),
);

const task = computed(() =>
  s.snapshot?.selected?.kind === "execution"
    ? s.snapshot.selected.value
    : s.snapshot?.requests.find((t) => t.action.requestId === s.taskId),
);
let polling: ReturnType<typeof setInterval>;
onMounted(() => {
  void c.refresh();
  polling = setInterval(() => {
    if (!s.busy) void c.refresh();
  }, 2000);
});
onUnmounted(() => {
  clearInterval(polling);
  c.dispose();
});

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
function stateLabel(value: string) {
  switch (value) {
    case "awaitingConfirmation":
      return "等待本人确认";
    case "ready":
      return "待执行";
    case "submitting":
      return "准备执行";
    case "failed":
      return "准备失败；没有创建新尝试";
    case "cancelled":
      return "已撤销";
    default:
      return "状态未知";
  }
}
function failureLabel(value: string) {
  switch (value) {
    case "riskUnknown":
      return "后台未提供可信风险等级，AI 操作已拒绝";
    case "riskBlocked":
      return "后台风险策略禁止 AI 执行此操作";
    case "preparationFailed":
      return "本机条件或后台 Start 未满足";
    case "interrupted":
      return "服务中断，未重新执行";
    case "expired":
      return "原任务已过期";
    case "revoked":
      return "后台授权已撤销";
    default:
      return "状态未知";
  }
}
</script>
<template>
  <div class="self-service">
    <div class="catalog-heading">
      <div>
        <h2>{{ s.page === "tasks" ? "执行服务记录" : "可用软件与工具" }}</h2>
        <p>浏览当前已加载目录，直接操作或向 AI 提问。</p>
      </div>
      <button :disabled="s.loading" @click="c.refresh">
        {{ s.loading ? "正在读取…" : "刷新" }}
      </button>
    </div>
    <p class="catalog-provenance" v-if="source === 's1'">
      S1 测试目录 · 样本资源 · 不代表真实安装或企业接线。
    </p>
    <p class="catalog-provenance" v-else-if="source === 'preview'">
      只读预览 · 请在桌面应用中连接执行服务。
    </p>
    <p class="catalog-provenance" v-else>
      当前执行服务返回的资源 · 不代表完整企业目录或已安装清单。
    </p>
    <p v-if="s.error" class="error" role="alert">{{ s.error }}</p>
    <section v-if="s.snapshot" aria-label="后台任务">
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
                  v-if="offer.summary.kind === 'software'"
                  :size="22"
                  aria-hidden="true" /><Terminal
                  v-else
                  :size="22"
                  aria-hidden="true"
              /></span>
              <span
                ><strong>{{ offer.title }}</strong
                ><small>{{
                  offer.summary.kind === "software"
                    ? "软件资源"
                    : "后台固定脚本"
                }}</small></span
              >
            </button>
            <p>分类与说明未提供</p>
            <span class="status-badge">{{
              source === "s1"
                ? "S1 样本"
                : source === "preview"
                  ? "预览"
                  : "执行服务目录"
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
          <h3 ref="resourceHeading" tabindex="-1">{{ resource.title }}</h3>
          <button type="button" @click="closeResource">关闭详情</button>
        </div>
        <p>
          {{ resource.summary.kind === "software" ? "软件详情" : "脚本详情" }} ·
          未提供分类 · 后台未提供资源说明
        </p>
        <ol v-if="resource.summary.kind === 'software'">
          <li v-for="(step, index) in resource.summary.steps" :key="index">
            {{ step.package }} · {{ step.version }} ·
            {{ step.identity === "system" ? "系统账号" : "当前用户" }}
          </li>
        </ol>
        <p v-else>
          运行身份：{{
            resource.summary.identity === "system" ? "系统账号" : "当前用户"
          }}
        </p>
        <p>
          有效期：{{ new Date(resource.expiresAt * 1000).toLocaleString() }}
        </p>
        <p>
          列表可见不代表已安装或获准执行；后台未提供完整能力判定，实际操作由执行服务裁决。
        </p>
        <div class="resource-actions">
          <button
            type="button"
            data-action="ask-ai"
            @click="
              emit('askAi', resource, $event.currentTarget as HTMLElement)
            "
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
        <h2>确认执行 {{ s.item.title }}</h2>
        <template v-if="s.item.summary.kind === 'software'">
          <p>操作：{{ operationLabel(s.item.summary.intent) }}</p>
          <ol>
            <li v-for="(step, index) in s.item.summary.steps" :key="index">
              {{ step.package }} {{ step.version }} ·
              {{ step.identity === "system" ? "系统账号" : "当前用户" }}
            </li>
          </ol>
        </template>
        <p v-else>
          后台固定脚本 ·
          {{ s.item.summary.identity === "system" ? "系统账号" : "当前用户" }}
        </p>
        <button :disabled="s.busy || s.uncertain" @click="c.confirm">
          确认并执行
        </button>
        <button :disabled="s.busy" @click="s.item = null">返回</button>
      </section>
      <section class="preparation-list" aria-label="准备中的请求">
        <h3 v-if="s.snapshot.preparations.length">准备中的请求</h3>
        <article
          v-for="pending in s.snapshot.preparations"
          :key="pending.offer.request"
        >
          <strong>{{ pending.offer.title }}</strong>
          <template v-if="pending.offer.summary.kind === 'software'">
            <p>操作：{{ operationLabel(pending.offer.summary.intent) }}</p>
            <ol>
              <li
                v-for="(step, index) in pending.offer.summary.steps"
                :key="index"
              >
                {{ step.package }} {{ step.version }} ·
                {{ step.identity === "system" ? "系统账号" : "当前用户" }}
              </li>
            </ol>
          </template>
          <p v-else>
            后台固定脚本 ·
            {{
              pending.offer.summary.identity === "system"
                ? "系统账号"
                : "当前用户"
            }}
          </p>
          <p v-if="pending.trigger.kind === 'ai'">
            AI 风险：{{ pending.risk?.level ?? "后台未提供" }}
          </p>
          <p>
            {{ stateLabel(pending.state)
            }}<span v-if="pending.failure">
              ·
              {{ failureLabel(pending.failure) }}</span
            >
          </p>
          <button
            v-if="pending.state === 'awaitingConfirmation'"
            :disabled="s.busy || s.uncertain"
            @click="c.confirmPreparation(pending.offer)"
          >
            确认此操作
          </button>
          <button
            v-if="!['failed', 'cancelled'].includes(pending.state)"
            :disabled="s.busy"
            @click="c.cancel(pending.offer.request)"
          >
            撤销请求
          </button>
        </article>
      </section>
      <section class="execution-records" aria-label="执行记录">
        <h3 v-if="s.snapshot.requests.length">执行记录</h3>
        <button
          v-for="record in s.snapshot.requests"
          :key="record.action.requestId"
          @click="s.taskId = record.action.requestId"
        >
          {{ record.action.operation.resource.id }} · {{ record.status.phase }}
        </button>
        <button v-if="s.previous.length" :disabled="s.loading" @click="c.first">
          首页
        </button>
        <button
          v-if="s.previous.length"
          :disabled="s.loading"
          @click="c.previous"
        >
          上一页
        </button>
        <button v-if="s.snapshot.next" :disabled="s.loading" @click="c.next">
          下一页
        </button>
      </section>
      <TaskDetail
        v-if="task"
        :task="task"
        :disabled="s.busy"
        @cancel="c.cancel(task.action.requestId)"
      />
    </section>
  </div>
</template>
