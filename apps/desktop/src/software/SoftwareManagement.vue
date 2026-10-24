<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { Package, UiSearchInput } from "@rss-mdm-agent/ui";
import type { PageNavigation } from "../workspace/page-navigation";
import type { Controller } from "../self-service/controller";
import type {
  BackendTask,
  InstalledSoftware,
  LocalSoftwareAction,
  SoftwareBlock,
} from "../self-service/types";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import LocalSoftwareActionView from "./LocalSoftwareAction.vue";
import {
  createSoftware,
  installationKey,
  type SoftwareController,
} from "./controller";
import { nativeDataPort, nativeSoftwarePort } from "../self-service/native";
import { currentUser } from "../test-users";
import "./software.css";
const props = defineProps<{
  model?: SoftwareController;
  execution?: Controller;
}>();
const emit = defineEmits<{
  navigation: [state: PageNavigation];
  navigate: [page: string];
}>();
const c = props.model ?? createSoftware(nativeDataPort(), nativeSoftwarePort());
const s = c.state;
const section = ref("installed"),
  query = ref(""),
  kind = ref(""),
  selected = ref(""),
  operation = ref("");
const heading = ref<HTMLElement>();
const sections = [
  { id: "installed", title: "已安装" },
  { id: "updates", title: "可更新" },
  { id: "enterprise", title: "企业软件" },
  { id: "operations", title: "本地操作记录" },
];
const kinds: Record<string, string> = {
  application: "macOS 应用",
  pkg: "PKG 安装收据",
  windowsProgram: "Windows 程序",
  msix: "MSIX 应用包",
  homebrew: "Homebrew 软件包",
};
const actionLabels: Record<LocalSoftwareAction, string> = {
  launch: "打开",
  update: "更新",
  uninstall: "卸载",
  detect: "检测",
};
const blocks: Record<SoftwareBlock, string> = {
  protected: "软件受到保护",
  ruleDisabled: "当前组织规则不允许",
  identityUnknown: "无法核实原生身份",
  observationUnavailable: "清单缺失、过期或不完整，请显式刷新",
  updateUnknown: "尚无获准的固定更新候选",
  noGui: "此安装记录没有可启动的 GUI",
  unsupported: "当前平台或安装机制不支持",
};
const filtered = computed(() =>
  c.installed.value.filter(
    (item) =>
      (!kind.value || item.kind === kind.value) &&
      (section.value !== "updates" ||
        item.actions.some(
          (a) => a.action === "update" && a.available && a.targetVersion,
        )) &&
      `${item.name} ${item.nativeId} ${item.publisher ?? ""}`
        .toLocaleLowerCase()
        .includes(query.value.trim().toLocaleLowerCase()),
  ),
);
const current = computed(() =>
  c.installed.value.find((item) => installationKey(item) === selected.value),
);
const offers = computed(
  () =>
    props.execution?.state.snapshot?.available.filter(
      (t) => t.presentation.summary.kind === "software",
    ) ?? [],
);
const history = computed(() =>
  [...s.operations.values()].sort(
    (a, b) => b.prepared.startBeforeUnixMs - a.prepared.startBeforeUnixMs,
  ),
);
function openSection(id: string) {
  if (sections.some((s) => s.id === id)) {
    section.value = id;
    selected.value = "";
  } else if (c.installed.value.some((item) => installationKey(item) === id)) {
    section.value = "installed";
    selected.value = id;
  } else if (id.startsWith("operation:")) {
    section.value = "operations";
    operation.value = id.slice(10);
  }
}
function reset() {
  section.value = "installed";
  selected.value = "";
  query.value = "";
  kind.value = "";
}
function navigation(): PageNavigation {
  return {
    sections,
    section: section.value,
    path: current.value
      ? [{ id: selected.value, title: current.value.name }]
      : [],
    resources: c.installed.value.map((item) => ({
      id: installationKey(item),
      title: item.name,
      description: `${kinds[item.kind]} · ${item.scope.kind === "device" ? "设备级" : "当前用户"}`,
      version: item.version ?? undefined,
      status: "上次采集时已安装",
      source: item.origin,
    })),
    searchEntries: c.installed.value.map((item) => ({
      id: installationKey(item),
      title: item.name,
    })),
    reset,
    openSection,
    setQuery: (text) => {
      query.value = text;
    },
  };
}
watch(
  [section, query, current, () => c.installed.value],
  () => emit("navigation", navigation()),
  { immediate: true },
);
async function inspect(item: InstalledSoftware) {
  selected.value = installationKey(item);
  await nextTick();
  heading.value?.focus();
}
async function act(item: InstalledSoftware, action: LocalSoftwareAction) {
  const prepared = await c.prepare(item, action);
  if (!prepared) return;
  operation.value = prepared.request;
  if (action === "launch" || action === "detect") await c.submit(prepared);
}
function backend(item: BackendTask) {
  props.execution?.select(item);
  emit("navigate", "software");
}
watch(currentUser, () => {
  reset();
  operation.value = "";
  c.dispose();
});
onMounted(() => {
  void c.discover();
  void c.history();
});
onBeforeUnmount(() => {
  if (!props.model) c.dispose();
});
</script>
<template>
  <div class="software-production design-page">
    <header class="catalog-heading">
      <div>
        <h2>软件管理</h2>
        <p>当前用户可见的安装记录与企业发布软件。</p>
      </div>
      <UiSearchInput
        v-model="query"
        placeholder="搜索已加载的软件"
        aria-label="搜索软件"
      />
    </header>
    <nav class="software-tabs" aria-label="软件视图">
      <button
        v-for="tab in sections"
        :key="tab.id"
        :aria-pressed="section === tab.id"
        @click="openSection(tab.id)"
      >
        {{ tab.title }}
      </button>
    </nav>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
    <p v-if="!c.port">当前环境无法连接本机软件服务，请在桌面应用中读取。</p>
    <template v-if="section === 'installed' || section === 'updates'">
      <p role="status">
        当前加载 {{ c.summary.value.installed }} 个安装项，{{
          c.summary.value.updates
        }}
        个获准的固定更新候选。{{
          c.summary.value.complete
            ? "已读取全部可见来源。"
            : "清单尚未完整；搜索与计数仅覆盖已加载范围。"
        }}
      </p>
      <label
        >安装类型
        <select v-model="kind">
          <option value="">全部类型</option>
          <option v-for="(name, id) in kinds" :key="id" :value="id">
            {{ name }}
          </option>
        </select></label
      >
      <details class="software-sources">
        <summary>数据来源与刷新</summary>
        <p>
          读取已保留结果不会采集新数据。点击“采集最新结果”才会提交明确的查询。
        </p>
        <article
          v-for="source in s.sources"
          :key="source.descriptor.reference.id"
        >
          <strong>{{ source.descriptor.title }}</strong> ·
          {{ source.page?.quality ?? "尚未读取" }}
          <span v-if="source.page?.collectedAt">
            ·
            {{
              new Date(source.page.collectedAt * 1000).toLocaleString()
            }}</span
          >
          <span v-if="source.page?.stale"> · 已过期</span
          ><span v-if="source.page && !source.page.enumerationComplete">
            · 枚举不完整</span
          >
          <p v-if="source.error" role="alert">{{ source.error }}</p>
          <button :disabled="source.busy" @click="c.read(source)">
            重读保留结果
          </button>
          <button
            v-if="source.after"
            :disabled="source.busy"
            @click="c.read(source, true)"
          >
            加载更多安装项
          </button>
          <button
            :disabled="!source.descriptor.available || source.busy"
            @click="c.refreshSource(source)"
          >
            采集最新结果
          </button>
        </article>
        <button :disabled="s.loading" @click="c.discover()">
          重新读取来源
        </button>
        <button
          v-if="s.directoryNext"
          :disabled="s.loading"
          @click="c.discover(true)"
        >
          加载更多来源
        </button>
        <div v-for="refresh in s.queries.values()" :key="refresh.request">
          <p>刷新原请求：{{ refresh.request }} · {{ refresh.phase }}</p>
          <button @click="c.queryRefresh(refresh.request)">查询刷新结果</button
          ><button
            :disabled="refresh.cancelRequested"
            @click="c.cancelRefresh(refresh.request)"
          >
            请求取消刷新
          </button>
        </div>
      </details>
      <div class="software-grid">
        <article
          v-for="item in filtered"
          :key="installationKey(item)"
          class="software-card"
        >
          <button class="software-title" @click="inspect(item)">
            <Package :size="24" aria-hidden="true" /><span
              ><strong>{{ item.name }}</strong
              ><small
                >{{ kinds[item.kind] }} ·
                {{ item.version ?? "版本未知" }}</small
              ></span
            >
          </button>
          <p>
            {{ item.scope.kind === "device" ? "设备级安装" : "当前用户安装" }} ·
            {{ item.publisher ?? "发布者未提供" }}
          </p>
          <button
            v-if="
              item.actions.some((a) => a.action === 'launch' && a.available)
            "
            :disabled="s.busy.has(installationKey(item))"
            @click="act(item, 'launch')"
          >
            打开
          </button>
          <button @click="inspect(item)">查看详情与操作</button>
        </article>
      </div>
      <p v-if="!filtered.length && !s.loading">当前加载范围内没有匹配软件。</p>
      <section v-if="current" class="software-detail" aria-label="软件详情">
        <h3 ref="heading" tabindex="-1">{{ current.name }}</h3>
        <p>
          {{ current.nativeId }} · {{ current.origin }} ·
          {{ current.scope.kind === "device" ? "设备级" : "当前用户" }}
        </p>
        <p>企业发布关联：未知。不会按名称匹配安装记录与企业授权。</p>
        <div v-for="action in current.actions" :key="action.action">
          <button
            :disabled="
              !action.available || s.busy.has(installationKey(current))
            "
            @click="act(current, action.action)"
          >
            {{ actionLabels[action.action]
            }}{{
              action.targetVersion ? `至 ${action.targetVersion}` : ""
            }}</button
          ><span v-if="action.reason">{{ blocks[action.reason] }}</span>
        </div>
      </section>
      <LocalSoftwareActionView
        v-if="operation"
        :key="operation"
        :request="operation"
        :port="c.port"
        @changed="c.accept"
      />
    </template>
    <template v-if="section === 'enterprise'">
      <PolicySelfService
        :port="execution?.port ?? null"
        :records="execution?.state.snapshot?.preparations ?? []"
        kind="software"
        :filter="query"
        @changed="execution?.refresh()"
      />
      <section aria-label="当前已签名的软件任务">
        <h3>当前已签名的软件任务</h3>
        <p>这是执行服务已提供的精确任务，安装状态和升级资格由执行服务核实。</p>
        <article v-for="offer in offers" :key="offer.request">
          <strong>{{ offer.presentation.title }}</strong
          ><button :disabled="!offer.userInitiated" @click="backend(offer)">
            查看并确认原任务
          </button>
        </article>
        <p v-if="!offers.length">当前没有已提供的软件任务。</p>
      </section>
    </template>
    <section v-if="section === 'operations'" aria-label="本地软件操作历史">
      <p v-if="s.historyError" role="alert">{{ s.historyError }}</p>
      <button :disabled="s.historyBusy" @click="c.history()">
        读取操作记录
      </button>
      <button
        v-if="s.historyNext"
        :disabled="s.historyBusy"
        @click="c.history(true)"
      >
        加载更多操作记录
      </button>
      <p>
        {{
          s.historyComplete ? "已读取当前历史页范围。" : "历史尚未读取完整。"
        }}
      </p>
      <button
        v-for="record in history"
        :key="record.prepared.request"
        @click="operation = record.prepared.request"
      >
        {{ record.prepared.name }} ·
        {{ actionLabels[record.prepared.choice.action] }} ·
        {{ record.execution.phase }}
      </button>
      <div v-for="request in s.requests" :key="request">
        <button v-if="!s.operations.has(request)" @click="operation = request">
          查询未确认的原请求 {{ request }}
        </button>
      </div>
      <LocalSoftwareActionView
        v-if="operation"
        :key="operation"
        :request="operation"
        :port="c.port"
        @changed="c.accept"
      />
    </section>
  </div>
</template>
