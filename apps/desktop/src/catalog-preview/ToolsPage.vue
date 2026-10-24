<script setup lang="ts">
import { reactive, computed, nextTick, ref, inject } from "vue";
import {
  Activity,
  Terminal,
  Monitor,
  Settings,
  ChevronRight,
  UiSearchInput,
} from "@rss-mdm-agent/ui";
import "./style.css";
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";
import ConnectionCheck from "../device-tools/ConnectionCheck.vue";
import type { NetworkCheckPort } from "../self-service/native";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import type { Controller } from "../self-service/controller";
import { systemSettingsKey } from "../system-settings/controller";
import { labels, display } from "../system-settings/presentation";
import { homeFacts } from "../system-settings/facts";
const props = withDefaults(
  defineProps<{
    execution?: Controller;
    active?: boolean;
    networkPort?: NetworkCheckPort | null;
  }>(),
  { active: true },
);
const systemSettings = inject(systemSettingsKey, undefined);
const facts = computed(() => homeFacts(systemSettings));
import type { PageNavigation } from "../workspace/page-navigation";
const emit = defineEmits<{
  navigation: [state: PageNavigation];
  navigate: [page: string, section?: string];
  task: [request: string];
}>();
const query = ref("");
const sections = [
  { id: "全部", title: "全部工具" },
  { id: "连接与网络", title: "连接与网络" },
  { id: "系统维护", title: "系统维护", kind: "tools" },
  { id: "安全与支持", title: "安全与支持", kind: "security" },
];
const section = computed(() => category.value);
const path = computed(() => [
  ...(category.value !== "全部"
    ? [{ id: category.value, title: category.value }]
    : []),
  ...(current.value
    ? [{ id: current.value.id, title: current.value.title }]
    : []),
]);
function reset() {
  category.value = "全部";
  selected.value = "";
  query.value = "";
}
function openSection(id: string) {
  if (id === "storage") id = "space";
  if (tools.some((e) => e.id === id)) {
    void inspect(id);
    return;
  }
  if (sections.some((e) => e.id === id)) {
    category.value = id;
    selected.value = "";
    query.value = "";
  }
}

const category = ref("全部");
const selected = ref("");
const heading = ref<HTMLElement>();
const listing = ref<HTMLElement>();
const tools = [
  {
    id: "network",
    title: "连接检查",
    category: "连接与网络",
    description: "应用连不上？逐项检查网络、DNS 与代理。",
    scope: "网络连接、地址与服务可达性",
    note: "先检查当前连接，再决定是否调整网络配置。",
    color: "blue",
    kind: "network",
    foot: "约 30 秒 · 只读检查",
  },
  {
    id: "space",
    title: "释放空间",
    category: "系统维护",
    description: "看看空间都用在哪里，再决定清理什么。",
    scope: "磁盘使用情况与可清理内容",
    note: "清理前逐项查看范围，不会默认删除文件。",
    color: "orange",
    kind: "storage",
    foot: "先查看，再清理",
  },
  {
    id: "power",
    title: "电源设置",
    category: "系统维护",
    description: "在续航与性能之间，找到适合你的平衡。",
    scope: "电池状态与供电偏好",
    note: "电源设置在电脑系统设置中调整。",
    color: "green",
    kind: "power",
    foot: "为当前工作状态调整",
  },
  {
    id: "security",
    title: "安全与权限",
    category: "安全与支持",
    description: "了解设备安全状态，检查常用应用权限。",
    scope: "系统保护与受管理配置",
    note: "组织限制和系统权限需要由相应管理方确认。",
    color: "violet",
    kind: "security",
    foot: "查看设备保护状态",
  },
  {
    id: "diagnostics",
    title: "收集诊断信息",
    category: "安全与支持",
    description: "整理必要信息，让 IT 更快了解问题。",
    scope: "Host、执行服务、设备与任务状态摘要",
    note: "导出前查看收集范围和敏感信息处理方式。",
    color: "rose",
    kind: "installed",
    foot: "预览内容后生成报告",
  },
  {
    id: "recent",
    title: "最近操作",
    category: "安全与支持",
    description: "安装、更新和诊断，每次操作都有迹可循。",
    scope: "已授权的请求与任务记录",
    note: "操作状态与执行结果分别核实。",
    color: "cyan",
    kind: "history",
    foot: "查看操作与结果",
  },
];
const filtered = computed(() =>
  tools.filter(
    (item) =>
      (category.value === "全部" || item.category === category.value) &&
      `${item.title}${item.description}`.includes(query.value.trim()),
  ),
);
const current = computed(() =>
  tools.find((item) => item.id === selected.value),
);
async function inspect(id: string) {
  selected.value = id;
  if (id === "space") void systemSettings?.load("storage");
  if (id === "network") void systemSettings?.load("network");
  await nextTick();
  heading.value?.focus();
}
async function back() {
  const id = selected.value;
  selected.value = "";
  await nextTick();
  Array.from(
    listing.value?.querySelectorAll<HTMLButtonElement>("button[data-tool]") ??
      [],
  )
    .find((button) => button.dataset.tool === id)
    ?.focus();
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
        description: item.description,
        status: item.foot,
        source: "工具目录",
      })),
    ),
    searchEntries: computed(() =>
      tools.map((e) => ({ id: e.id, title: e.title })),
    ),
    setQuery: (value: string) => {
      query.value = value;
      selected.value = "";
    },
  }),
);
</script>
<template>
  <section class="catalog-preview design-page" aria-label="工具中心">
    <div class="catalog-stage">
      <template v-if="current"
        ><button class="preview-back" type="button" @click="back">
          ‹ 返回工具目录
        </button>
        <article class="software-detail">
          <div class="software-detail-heading">
            <span class="tool-icon" :class="current.color"
              ><SettingsGlyph :kind="current.kind" :size="28"
            /></span>
            <div>
              <h3 ref="heading" tabindex="-1">{{ current.title }}</h3>
              <p>{{ current.category }}</p>
            </div>
          </div>
          <p>{{ current.description }}</p>
          <div class="tool-scope">
            <h4>将检查哪些内容</h4>
            <p>{{ current.scope }}</p>
            <h4>开始之前</h4>
            <p>{{ current.note }}</p>
          </div>
          <section v-if="current.id === 'space'" aria-label="磁盘使用情况">
            <p>可用 {{ facts.storageFree }} GB · {{ facts.storageDetail }}</p>
            <p v-if="systemSettings?.state.errors.get('storage')" role="alert">
              {{ systemSettings.state.errors.get("storage") }}
            </p>
            <p>
              清理范围由组织发布的工具说明。当前页面提供空间查看；请在工具目录选择已授权的清理脚本。
            </p>
            <button
              :disabled="
                !systemSettings || systemSettings.state.busy.has('storage')
              "
              @click="systemSettings?.load('storage', true)"
            >
              {{
                systemSettings?.state.busy.has("storage")
                  ? "正在刷新空间信息…"
                  : "刷新空间信息"
              }}</button
            ><button @click="back">查看组织发布的工具</button>
          </section>
          <section v-if="current.id === 'network'" aria-label="网络与代理事实">
            <p>{{ facts.network }} · {{ facts.networkDetail }}</p>
            <p v-if="systemSettings?.state.errors.get('network')" role="alert">
              {{ systemSettings.state.errors.get("network") }}
            </p>
            <button
              :disabled="
                !systemSettings || systemSettings.state.busy.has('network')
              "
              @click="systemSettings?.load('network', true)"
            >
              {{
                systemSettings?.state.busy.has("network")
                  ? "正在刷新网络状态…"
                  : "刷新网络状态"
              }}
            </button>
            <template
              v-if="systemSettings?.state.observations.get('network')?.snapshot"
            >
              <p>
                本机观察时间：{{
                  new Date(
                    systemSettings.state.observations.get(
                      "network",
                    )!.snapshot!.collectedAtUnixMs,
                  ).toLocaleString()
                }}
              </p>
              <article
                v-for="target in systemSettings.state.observations.get(
                  'network',
                )!.snapshot!.targets"
                :key="target.id"
              >
                <strong>{{ target.name }}</strong>
                <p v-for="field in target.fields" :key="field.key">
                  {{ labels[field.key] ?? field.key }}：{{
                    field.value == null ? "未读取" : display(field.value)
                  }}
                  · {{ field.quality
                  }}{{ field.reason ? " · " + field.reason : "" }}
                </p>
              </article>
            </template>
            <p v-else>尚无本机网络或代理观察，请显式刷新。</p>
          </section>
          <button
            v-if="current.id !== 'network'"
            class="preview-primary"
            type="button"
            @click="
              emit(
                'navigate',
                current.id === 'space' ||
                  current.id === 'power' ||
                  current.id === 'security'
                  ? 'system-settings'
                  : current.id === 'recent'
                    ? 'tasks'
                    : 'settings',
                current.id === 'space'
                  ? 'storage'
                  : current.id === 'power'
                    ? 'power'
                    : current.id === 'security'
                      ? 'security'
                      : current.id === 'diagnostics'
                        ? 'diagnostics'
                        : undefined,
              )
            "
          >
            {{
              current.id === "space" ||
              current.id === "power" ||
              current.id === "security"
                ? "打开对应系统设置"
                : current.id === "recent"
                  ? "查看最近操作"
                  : "预览并导出诊断"
            }}
            <ChevronRight :size="14" aria-hidden="true" />
          </button></article
      ></template>
      <template v-else>
        <div class="design-hero tools-editorial">
          <div>
            <span class="design-eyebrow">A LITTLE CARE, A BETTER DAY</span>
            <h2>电脑的小状况，<br />这里有好办法。</h2>
            <p>
              检查连接、整理空间、照顾电池。<br />把处理问题的时间，留给更重要的事。
            </p>
            <button class="design-button" @click="inspect('network')">
              从连接检查开始 →
            </button>
          </div>
          <div class="tool-orbit" aria-hidden="true">
            <span><SettingsGlyph kind="power" :size="27" /></span
            ><span><SettingsGlyph kind="tools" :size="51" /></span
            ><span><SettingsGlyph kind="security" :size="25" /></span>
          </div>
        </div>

        <div ref="listing" class="tools-grid">
          <button
            v-for="item in filtered"
            :key="item.id"
            type="button"
            :data-tool="item.id"
            @click="inspect(item.id)"
          >
            <span class="tool-card-top"
              ><span class="tool-icon" :class="item.color"
                ><SettingsGlyph :kind="item.kind" :size="21" /></span
              ><span class="tool-availability">{{ item.category }}</span></span
            ><strong>{{ item.title }}</strong>
            <p>{{ item.description }}</p>
            <span class="tool-card-link"
              >{{ item.foot }} <ChevronRight :size="13" aria-hidden="true"
            /></span>
          </button>
        </div>
        <p v-if="!filtered.length" role="status" class="catalog-empty">
          没有匹配的工具，请换个关键词。
        </p>
        <section aria-label="组织发布的工具">
          <h3>组织发布的工具</h3>
          <PolicySelfService
            v-if="active"
            :port="execution?.port ?? null"
            :records="execution?.state.snapshot?.preparations ?? []"
            kind="script"
            :filter="query"
            @changed="execution?.refresh()"
          />
        </section>
        <div class="organization-tools">
          <SettingsGlyph kind="about" :size="19" />
          <div>
            <strong>有些问题，交给熟悉它的人。</strong>
            <p>查看服务状态或导出脱敏诊断，便于你的 IT 支持继续排查。</p>
          </div>
          <button @click="emit('navigate', 'settings', 'diagnostics')">
            查看诊断信息 ↗
          </button>
        </div>
      </template>
      <ConnectionCheck
        v-show="current?.id === 'network'"
        :port="networkPort ?? null"
        :active="active && current?.id === 'network'"
        @task="emit('task', $event)"
      />
    </div>
  </section>
</template>
