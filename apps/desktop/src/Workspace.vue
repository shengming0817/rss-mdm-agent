<script setup lang="ts">
import {
  computed,
  provide,
  nextTick,
  ref,
  shallowRef,
  onBeforeUnmount,
  onMounted,
  watch,
} from "vue";
import {
  UiNavigation,
  UiBadge,
  UiIcon,
  House,
  Package,
  Terminal,
  Sparkles,
  Settings as SettingsIcon,
  X,
} from "@rss-mdm-agent/ui";
import AppShell from "./workspace/WorkspaceChrome.vue";
import WorkspaceSearch from "./workspace/WorkspaceSearch.vue";
import SettingsGlyph from "./system-settings/SettingsGlyph.vue";
import AccountMenu from "./workspace/AccountMenu.vue";
import ConversationShortcut from "./workspace/ConversationShortcut.vue";
import ModuleNavigation from "./workspace/ModuleNavigation.vue";
import "./workspace/design.css";
import ConversationHistory from "./workspace/ConversationHistory.vue";
import PageAssistantPanel from "./workspace/PageAssistantPanel.vue";
import { homeFacts } from "./system-settings/facts";
import { pageContext } from "./workspace/page-context";
import { nativeAssistant } from "./assistant/native";
import Assistant from "./assistant/Assistant.vue";
import type { SelfServicePort } from "./self-service/types";
import {
  createAssistant,
  type AssistantServices,
} from "./assistant/controller";
import { createActivity, activityKey } from "./self-service/activity";
import Tasks from "./self-service/Tasks.vue";
import { createDirectory, directoryKey } from "./self-service/directory";
import type { LocalPorts } from "./assembly";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import Home from "./home/Home.vue";
import SystemSettings from "./system-settings/SystemSettings.vue";
import {
  createSystemSettings,
  systemSettingsKey,
} from "./system-settings/controller";
import SoftwareManagement from "./software/SoftwareManagement.vue";
import { createSoftware, softwareSummaryKey } from "./software/controller";
import {
  nativeDataPort,
  nativeSoftwarePort,
  nativeSettingsPort,
  nativeNetworkCheckPort,
} from "./self-service/native";
import ToolsPage from "./device-tools/ToolsPage.vue";
import { createDeviceOverview, type DeviceOverviewPort } from "./home/device";
import { nativeDeviceOverview } from "./home/native";
import { createController } from "./self-service/controller";
import { nativePort } from "./self-service/native";
import Settings from "./settings/Settings.vue";
import type { ServicePort } from "./settings/native";
import type { HostSettings } from "./settings/controller";
import "./self-service/style.css";
import "./assistant/style.css";
const props = defineProps<{
  ready: boolean;
  identity?: UserContext;
  localPorts?: LocalPorts;
  busy: boolean;
  assistantServices?: AssistantServices;
  selfServicePort?: SelfServicePort;
  page: string;
  host: HostSettings;
  servicePort?: ServicePort;
  fixture?: boolean;
  devicePort?: DeviceOverviewPort;
}>();
const emit = defineEmits<{
  navigate: [id: string];
}>();
import { workspaceIdentity } from "./workspace/identity";
import type {
  PageNavigation,
  NavigationTarget,
  SearchEntry,
} from "./workspace/page-navigation";
const softwarePage = ref<PageNavigation>();
const toolsPage = ref<PageNavigation>();
const systemPage = ref<PageNavigation>();
const settingsPage = ref<PageNavigation>();
const settingsExpanded = ref(false);
const chromeNarrow = ref(false),
  searchRequest = ref(0);
const designedPages = [
  "home",
  "settings",
  "system-settings",
  "software-management",
  "device-tools",
];
const currentModule = computed(() =>
  props.page === "system-settings"
    ? systemPage.value
    : props.page === "software-management"
      ? softwarePage.value
      : props.page === "device-tools"
        ? toolsPage.value
        : props.page === "settings"
          ? settingsPage.value
          : undefined,
);
const moduleTitle = computed(
  () =>
    [
      { id: "settings", title: "RSS Agent 设置" },
      { id: "system-settings", title: "系统设置" },
      { id: "software-management", title: "软件管理" },
      { id: "device-tools", title: "工具中心" },
      { id: "home", title: "主页" },
    ].find((item) => item.id === props.page)?.title ?? "",
);
const searchEntries = computed<SearchEntry[]>(() => [
  ...[
    {
      page: "software-management",
      group: "软件管理",
      module: softwarePage.value,
    },
    { page: "device-tools", group: "工具", module: toolsPage.value },
    { page: "system-settings", group: "系统设置", module: systemPage.value },
    { page: "settings", group: "RSS Agent 设置", module: settingsPage.value },
  ].flatMap(({ page, group, module }) =>
    (module?.sections ?? []).map((e) => ({
      ...e,
      kind: "section" as const,
      page,
      group,
    })),
  ),
  ...(softwarePage.value?.searchEntries ?? []).map((e) => ({
    ...e,
    kind: "resource" as const,
    page: "software-management",
    group: "应用",
  })),
  ...(toolsPage.value?.searchEntries ?? []).map((e) => ({
    ...e,
    kind: "resource" as const,
    page: "device-tools",
    group: "工具",
  })),
  {
    kind: "section" as const,
    page: "system-settings",
    id: "lock",
    title: "屏幕与睡眠",
    group: "系统设置 · 电源",
  },
]);
const navigationError = ref("");
const searchState = computed(() => {
  const states = [softwarePage.value?.search, toolsPage.value?.search].filter(
    (value) => !!value,
  );
  return {
    loading: states.some((value) => value!.loading),
    complete: states.length === 2 && states.every((value) => value!.complete),
    hasMore: states.some((value) => value!.hasMore),
    error: states
      .map((value) => value!.error)
      .filter((value) => !!value)
      .join(" · "),
  };
});
async function searchMore(retry = false) {
  if (searchState.value.loading) return;
  if (retry) {
    if (
      software.state.error ||
      software.state.sources.some((source) => source.error)
    ) {
      if (software.state.error) await software.discover();
      for (const source of software.state.sources.filter(
        (source) => source.error,
      ))
        await software.read(source, !!source.after);
    }
    if (directory.state.error) await directory.load(!!directory.state.next);
  } else {
    // Software and tools consume the same Policy owner; advance that cursor once.
    if (software.state.directoryNext) await software.discover(true);
    else {
      const source = software.state.sources.find((source) => source.after);
      if (source) await software.read(source, true);
    }
    if (directory.state.next) await directory.load(true);
  }
}
let navigationEpoch = 0;
async function openTarget(target: NavigationTarget) {
  const call = ++navigationEpoch;
  navigationError.value = "";
  if (target.kind === "request") {
    await openTask(target.request);
    return;
  }
  const module =
    target.page === "software-management"
      ? softwarePage.value
      : target.page === "device-tools"
        ? toolsPage.value
        : target.page === "system-settings"
          ? systemPage.value
          : target.page === "settings"
            ? settingsPage.value
            : undefined;
  if (!module) {
    navigationError.value = "目标页面不可用，请刷新后重试。";
    return;
  }
  if (
    target.kind === "resource" &&
    !module.searchEntries?.some(
      (item) => item.id === target.id && item.revision === target.revision,
    )
  ) {
    navigationError.value = "资源已撤回或版本已变化，请重新搜索。";
    return;
  }
  navigateMain(target.page);
  await nextTick();
  if (call !== navigationEpoch) return;
  if (target.kind === "resource") {
    if (!(await module.openResource?.(target.id, target.revision)))
      navigationError.value = "资源不可访问，请重新读取当前目录。";
  } else if (
    module.sections.some((item) => item.id === target.id) ||
    (target.page === "system-settings" && target.id === "lock")
  )
    module.openSection(target.id);
  else navigationError.value = "目标分区已变化，请重新选择。";
}
function resetModule(id: string) {
  if (id === "software-management") softwarePage.value?.reset();
  if (id === "device-tools") toolsPage.value?.reset();
  if (id === "system-settings") systemPage.value?.reset();
  if (id === "settings") settingsPage.value?.reset();
}
function navigateMain(id: string) {
  settingsExpanded.value = false;
  emit("navigate", id);
}
function accountMenu(id: string) {
  settingsExpanded.value = true;
  settingsPage.value?.openSection(id);
  emit("navigate", "settings");
}
function openConnectionSettings() {
  accountMenu("connection");
}
async function openTask(request: string) {
  if (!controller) return;
  const details = activity.state.items.find(
    (item) => item.details.action.requestId === request,
  )?.details;
  const action = details?.action.operation.action;
  if (
    [
      "local-software.launch",
      "local-software.uninstall",
      "local-software.detect",
    ].includes(action ?? "")
  ) {
    navigateMain("software-management");
    await nextTick();
    await softwarePage.value?.openRequest?.(request);
    return;
  }
  if (action === "network.check") {
    navigateMain("device-tools");
    await nextTick();
    await toolsPage.value?.openRequest?.(request);
    return;
  }
  controller.state.taskId = request;
  contextualNavigate("tasks");
  await controller.refresh();
}
function contextualNavigate(page: string, section?: string) {
  navigateMain(page);
  if (section) {
    const module =
      page === "system-settings"
        ? systemPage.value
        : page === "software-management"
          ? softwarePage.value
          : page === "device-tools"
            ? toolsPage.value
            : page === "settings"
              ? settingsPage.value
              : undefined;
    module?.openSection(section);
  }
}
const historyOpen = ref(false),
  historyPinned = ref(false),
  historyQuery = ref("");
const historyPanel = ref<HTMLElement>();
async function closeHistory(restore = false) {
  historyOpen.value = false;
  historyPinned.value = false;
  if (!restore) return;
  await nextTick();
  const triggers = workspaceRoot.value
    ?.closest(".workspace-chrome")
    ?.querySelectorAll<HTMLElement>('[data-action="history-conversations"]');
  [...(triggers ?? [])]
    .find((button) => button.getClientRects().length)
    ?.focus({ preventScroll: true });
}
async function toggleHistory() {
  if (historyOpen.value) {
    closeHistory();
    return;
  }
  historyPinned.value = false;
  historyOpen.value = true;
  await nextTick();
  historyPanel.value?.querySelector<HTMLInputElement>("input")?.focus();
}
function dismissHistory(event: PointerEvent) {
  if (!historyOpen.value || historyPinned.value) return;
  const node = event.target as HTMLElement | null;
  if (
    !node?.closest(
      '.conversation-history-panel, [data-action="history-conversations"], .sidebar-cycle',
    )
  )
    closeHistory();
}
async function historySelected() {
  const floating = !historyPinned.value;
  if (floating) closeHistory();
  if (!pageAssistantOpen.value) emit("navigate", "assistant");
  if (floating) {
    await nextTick();
    await nextTick();
    workspaceRoot.value
      ?.querySelector<HTMLTextAreaElement>(".composer textarea")
      ?.focus({ preventScroll: true });
  }
}
const device = createDeviceOverview(
  props.fixture ? props.devicePort : nativeDeviceOverview(),
);
let devicePolling: ReturnType<typeof setInterval> | undefined;
let taskPolling: ReturnType<typeof setInterval> | undefined;
const lastContentPage = ref("home");
const newIdentity = () => crypto.randomUUID();
const ownerKey = workspaceIdentity(props.identity);
const local: LocalPorts = props.localPorts ?? {
  data: !props.ready || props.fixture ? null : nativeDataPort(),
  software: !props.ready || props.fixture ? null : nativeSoftwarePort(),
  settings: !props.ready || props.fixture ? null : nativeSettingsPort(),
  network: !props.ready || props.fixture ? null : nativeNetworkCheckPort(),
};
const software = createSoftware(local.data, local.software);
provide(softwareSummaryKey, software.summary);
const networkCheck = local.network;
const systemSettings = createSystemSettings(local.settings, local.data);
provide(systemSettingsKey, systemSettings);
const controller = props.ready
  ? createController(props.selfServicePort ?? nativePort())
  : undefined;
const directory = createDirectory(controller?.port ?? null);
provide(directoryKey, directory);
const activity = createActivity(controller?.port ?? null);
provide(activityKey, activity);
onMounted(() => {
  for (const section of ["network", "storage", "power"] as const)
    void systemSettings.load(section);
});
onBeforeUnmount(() => {
  systemSettings.dispose();
  software.dispose();
  directory.dispose();
  activity.dispose();
  controller?.dispose();
});
const assistant = props.ready
  ? createAssistant(
      props.assistantServices ?? nativeAssistant(),
      newIdentity,
      () => Date.now(),
      ownerKey,
    )
  : undefined;
watch(
  () => props.page,
  (id) => {
    if (id !== "settings") settingsExpanded.value = false;
    if (id !== "assistant") lastContentPage.value = id;
    navigationError.value = "";
    if (id === "home") {
      void device.refresh();
      void activity.load();
    }
    if (["tasks", "software-management", "device-tools"].includes(id))
      void controller?.refresh();
    if (!historyPinned.value) historyOpen.value = false;
  },
  { immediate: true },
);
const moving = ref(false);
const target = shallowRef<HTMLElement>();
const pageAssistantOpen = ref(false);
const pagePanelHost = ref<HTMLElement>();
const pageAssistantTrigger = ref<HTMLButtonElement>();
const homeResources = computed(() => {
  const facts = homeFacts(systemSettings),
    summary = software.summary.value;
  return [
    {
      id: "computer",
      title: device.state.value?.attributes.computerName ?? "这台电脑",
      description: "当前设备概览",
      source: "设备概览",
      revision: JSON.stringify(device.state.value),
      collectedAt: device.state.readAt ?? undefined,
      validUntil:
        device.state.readAt == null ? 0 : device.state.readAt + 30_000,
      verified: !!device.state.value,
    },
    ...[
      ["network", "网络", facts.network, facts.networkDetail],
      ["storage", "存储", facts.storageFree, facts.storageDetail],
      ["power", "电源", facts.battery, facts.batteryDetail],
    ].map(([id, title, value, detail]) => ({
      id,
      title,
      description: `${value} · ${detail}`,
      source: "本地已采集结果",
      revision: JSON.stringify([value, detail]),
      verified: !systemSettings.state.errors.has(
        id as "network" | "storage" | "power",
      ),
    })),
    {
      id: "software",
      title: "已安装应用",
      description: summary.read
        ? `已加载 ${summary.installed} 项 · ${summary.updates} 个固定更新候选 · ${summary.complete ? "完整清单" : "部分或过期清单"}`
        : "清单未读取",
      source: "软件业务投影",
      revision: JSON.stringify(summary.items.map((item) => item.reference)),
      collectedAt:
        summary.collectedAt == null ? undefined : summary.collectedAt * 1000,
      validUntil:
        summary.collectedAt == null ? 0 : (summary.collectedAt + 300) * 1000,
      verified: summary.read && !summary.error,
    },
    ...activity.state.items.slice(0, 6).map((item) => ({
      id: item.details.action.requestId,
      title: item.details.action.operation.action,
      description: item.details.status.phase,
      source: "原任务 journal",
      revision: JSON.stringify([
        item.details.action.contentDigest,
        item.details.status,
      ]),
      collectedAt: item.recordedAtUnixMs,
      verified: !activity.state.error,
    })),
  ];
});
const pageContexts = computed(() =>
  [
    ["software-management", "软件管理", softwarePage.value],
    ["device-tools", "工具中心", toolsPage.value],
    ["system-settings", "系统设置", systemPage.value],
    ["settings", "RSS Agent 设置", settingsPage.value],
    ["home", "主页", undefined],
  ].map(([page, title, module]) => {
    const navigation = module as PageNavigation | undefined;
    return pageContext(
      page as string,
      title as string,
      navigation?.path ?? [],
      navigation?.resources ?? (page === "home" ? homeResources.value : []),
      ownerKey,
    );
  }),
);
const currentPageContext = computed(
  () =>
    pageContexts.value.find(
      (value) =>
        value.path[0] ===
        (props.page === "assistant"
          ? [
              { id: "home", title: "主页" },
              { id: "software-management", title: "软件管理" },
              { id: "device-tools", title: "工具中心" },
              { id: "system-settings", title: "系统设置" },
              { id: "settings", title: "RSS Agent 设置" },
            ].find((item) => item.id === lastContentPage.value)?.title
          : moduleTitle.value),
    ) ??
    pageContext(
      props.page,
      moduleTitle.value || "请求与任务",
      [],
      [],
      ownerKey,
    ),
);
function attachPage() {
  assistant?.stageContext(currentPageContext.value);
}
async function togglePageAssistant() {
  if (!assistant) return;
  const opening = !pageAssistantOpen.value;
  if (opening) closeHistory();
  if (opening && props.page === "assistant")
    emit("navigate", lastContentPage.value);
  await relocate(() => {
    pageAssistantOpen.value = opening;
  });
  if (opening) {
    attachPage();
    pagePanelHost.value
      ?.querySelector<HTMLTextAreaElement>("textarea")
      ?.focus();
  } else pageAssistantTrigger.value?.focus();
}
async function openMainAssistant() {
  await relocate(() => {
    pageAssistantOpen.value = false;
  });
  emit("navigate", "assistant");
}
watch(
  pageContexts,
  (values) => {
    assistant?.validatePageContexts(values);
  },
  { immediate: true },
);
const headerHost = ref<HTMLElement>();
let transition = 0;
const assistantVisible = computed(
  () =>
    !moving.value && (props.page === "assistant" || pageAssistantOpen.value),
);
async function relocate(change: () => void) {
  const version = ++transition;
  moving.value = true;
  target.value = undefined;
  await nextTick();
  if (version !== transition) return;
  change();
  await nextTick();
  if (version !== transition) return;
  if (pageAssistantOpen.value) target.value = pagePanelHost.value;
  moving.value = false;
}
watch(
  () => props.page,
  () => {
    if (props.page === "assistant" && pageAssistantOpen.value)
      void relocate(() => {
        pageAssistantOpen.value = false;
      });
  },
);
let sizing: ResizeObserver | undefined;
const workspaceRoot = ref<HTMLElement>();
const attention = computed(() => assistant?.attention.value ?? 0);
const scrollPositions = new Map<string, number>();
const pageScroller = (page: string) => {
  const root = workspaceRoot.value;
  const selector =
    page === "home"
      ? ".home-content"
      : page === "software-management"
        ? ".software-production"
        : page === "device-tools"
          ? ".catalog-preview"
          : page === "system-settings"
            ? ".system-settings"
            : page === "settings"
              ? ".settings"
              : ".resource-content";
  return root?.querySelector<HTMLElement>(selector) ?? root?.closest("main");
};
watch(
  () => props.page,
  async (page, previous) => {
    const scroller = previous ? pageScroller(previous) : undefined;
    if (previous && scroller) scrollPositions.set(previous, scroller.scrollTop);
    await nextTick();
    if (props.page !== page) return;
    const destination = pageScroller(page);
    if (destination) destination.scrollTop = scrollPositions.get(page) ?? 0;
  },
  { flush: "pre" },
);
onMounted(() => {
  void device.refresh();
  devicePolling = setInterval(() => {
    if (props.page === "home" && !document.hidden) void device.refresh();
  }, 30000);
  taskPolling = setInterval(() => {
    if (!document.hidden && props.page === "home") void activity.load();
    if (
      !document.hidden &&
      ["tasks", "software-management", "device-tools"].includes(props.page) &&
      !controller?.state.busy
    )
      void controller?.refresh();
  }, 2000);
  void controller?.refresh();
  void assistant?.connect();
  if (workspaceRoot.value) {
    const resize = () => {
      const outer =
        workspaceRoot.value!.closest(".workspace-chrome") ??
        workspaceRoot.value!;
      chromeNarrow.value = outer.getBoundingClientRect().width < 900;
    };
    resize();
    sizing = new ResizeObserver(resize);
    sizing.observe(workspaceRoot.value);
    sizing.observe(
      workspaceRoot.value.closest(".workspace-chrome") ?? workspaceRoot.value,
    );
  }
});
onBeforeUnmount(() => {
  if (devicePolling) clearInterval(devicePolling);
  if (taskPolling) clearInterval(taskPolling);
  device.dispose();
  transition++;
  navigationEpoch++;
  sizing?.disconnect();
  assistant?.dispose();
});
</script>
<template>
  <AppShell
    @pointerdown.capture="dismissHistory"
    @keydown.esc="historyOpen && !historyPinned && closeHistory(true)"
    :designed="designedPages.includes(page)"
    :narrow="chromeNarrow"
    @search="searchRequest++"
    :navigation-enabled="true"
    :navigation-key="page"
    :content-mode="
      ready &&
      ![
        'settings',
        'system-settings',
        'software-management',
        'device-tools',
      ].includes(page)
        ? 'conversation'
        : 'page'
    "
  >
    <template #header>
      <div class="workspace-page-header" data-tauri-drag-region>
        <div
          ref="headerHost"
          class="assistant-header-host"
          :hidden="page !== 'assistant'"
        />
        <nav
          v-if="ready && designedPages.includes(page)"
          class="workspace-breadcrumb"
          aria-label="页面路径"
        >
          <button type="button" @click="resetModule(page)">
            {{ moduleTitle }}</button
          ><template
            v-for="(item, index) in currentModule?.path ?? []"
            :key="item.id"
            ><span aria-hidden="true">/</span
            ><button
              v-if="index < (currentModule?.path.length ?? 0) - 1"
              type="button"
              @click="currentModule?.openSection(item.id)"
            >
              {{ item.title }}</button
            ><span v-else aria-current="page">{{ item.title }}</span></template
          >
        </nav>
        <h1 v-else-if="page !== 'assistant'" tabindex="-1">
          {{
            !ready || page === "settings"
              ? "设置"
              : page === "software-management"
                ? "软件管理"
                : page === "device-tools"
                  ? "工具"
                  : page === "system-settings"
                    ? "系统设置"
                    : page === "tasks"
                      ? "请求与任务"
                      : "首页"
          }}
        </h1>
      </div>
    </template>
    <template #search>
      <WorkspaceSearch
        v-if="ready"
        :entries="searchEntries"
        :request="searchRequest"
        :loading="searchState.loading"
        :complete="searchState.complete"
        :has-more="searchState.hasMore"
        :error="searchState.error"
        @more="searchMore()"
        @retry="searchMore(true)"
        @select="openTarget"
      />
    </template>
    <template #actions>
      <button
        v-if="ready"
        ref="pageAssistantTrigger"
        aria-label="打开电脑助手"
        :aria-expanded="pageAssistantOpen"
        title="电脑助手"
        class="workspace-assistant-entry"
        type="button"
        @click="togglePageAssistant"
      >
        <Sparkles :size="16" aria-hidden="true" />
      </button>
    </template>
    <template #navigation="{ navigate, layout, inlineSecondary }"
      ><UiNavigation
        class="workspace-primary-navigation"
        label="工作区导航"
        :items="[
          { id: 'home', label: '主页' },
          { id: 'assistant', label: 'AI 助手' },
          { id: 'software-management', label: '软件管理' },
          { id: 'device-tools', label: '工具中心' },
          { id: 'system-settings', label: '系统设置' },
        ]"
        :model-value="page"
        @update:model-value="
          (id) => {
            navigateMain(id);
            navigate();
          }
        "
      >
        <template #icon="{ item }"
          ><UiIcon :size="22" :title="item.label"
            ><Sparkles v-if="item.id === 'assistant'" /><House
              v-else-if="item.id === 'home'" /><SettingsIcon
              v-else-if="item.id === 'system-settings'" /><Package
              v-else-if="item.id === 'software-management'" /><SettingsGlyph
              v-else
              kind="tools"
              :size="22" /></UiIcon
        ></template>
        <template #trailing="{ item }"
          ><UiBadge
            v-if="item.id === 'assistant' && attention"
            :aria-label="`待回应 ${attention}`"
            >{{ attention }}</UiBadge
          ></template
        >
        <template #after-item="{ item }"
          ><div
            v-if="inlineSecondary && item.id === page && page !== 'home'"
            class="navigation-children"
          >
            <template v-if="page === 'assistant'"
              ><ConversationShortcut
                v-if="assistant"
                :controller="assistant"
                :history-open="historyOpen"
                @select="
                  emit('navigate', 'assistant');
                  navigate();
                "
                @history="
                  toggleHistory();
                  navigate();
                " /></template
            ><ModuleNavigation
              v-else
              :items="currentModule?.sections ?? []"
              :selected="currentModule?.section ?? ''"
              @select="
                currentModule?.openSection($event);
                navigate();
              "
            /></div
        ></template> </UiNavigation
    ></template>
    <template #secondary>
      <ModuleNavigation
        v-if="settingsExpanded"
        :items="settingsPage?.sections ?? []"
        :selected="settingsPage?.section ?? ''"
        @select="accountMenu"
      />
      <ConversationShortcut
        v-else-if="page === 'assistant' && assistant"
        :controller="assistant"
        :history-open="historyOpen"
        @history="toggleHistory"
        @select="emit('navigate', 'assistant')"
      />
      <ModuleNavigation
        v-else
        :items="currentModule?.sections ?? []"
        :selected="currentModule?.section ?? ''"
        @select="currentModule?.openSection($event)"
      />
    </template>
    <template #navigation-footer="{ navigate }">
      <div class="workspace-settings-footer">
        <ModuleNavigation
          v-if="settingsExpanded"
          class="agent-settings-navigation"
          :items="settingsPage?.sections ?? []"
          :selected="settingsPage?.section ?? ''"
          @select="
            (id) => {
              accountMenu(id);
              navigate();
            }
          "
        />
        <AccountMenu
          :expanded="settingsExpanded"
          :active="page === 'settings'"
          @open="accountMenu('account')"
          ><slot name="account-summary">账户与连接</slot></AccountMenu
        >
      </div></template
    >
    <div
      ref="workspaceRoot"
      class="workspace-content"
      :class="{ 'history-reserved': historyOpen && historyPinned }"
      :inert="busy ? true : undefined"
    >
      <aside
        v-if="assistant && historyOpen"
        ref="historyPanel"
        class="conversation-history-panel"
        :class="{ 'history-pinned': historyPinned }"
        :role="historyPinned ? 'complementary' : 'dialog'"
        :aria-modal="historyPinned ? undefined : false"
        aria-label="历史会话清单"
        tabindex="-1"
      >
        <div class="history-heading">
          <strong>历史会话</strong>
          <div class="history-actions">
            <button
              type="button"
              class="icon-button"
              :aria-label="historyPinned ? '取消固定历史会话' : '固定历史会话'"
              :aria-pressed="historyPinned"
              @click="historyPinned = !historyPinned"
            >
              <svg
                viewBox="0 0 24 24"
                width="16"
                height="16"
                fill="none"
                stroke="currentColor"
                stroke-width="1.7"
                aria-hidden="true"
              >
                <path d="M9 3h6l1 7 3 3v2H5v-2l3-3 1-7Zm3 12v7" />
              </svg></button
            ><button
              type="button"
              class="icon-button"
              aria-label="关闭历史会话"
              @click.stop="closeHistory(true)"
            >
              <X :size="16" />
            </button>
          </div>
        </div>
        <ConversationHistory
          v-model:query="historyQuery"
          :controller="assistant"
          @select="historySelected"
        />
      </aside>
      <p v-if="navigationError" role="alert">{{ navigationError }}</p>
      <div class="workspace-page-surface">
        <div
          v-if="controller"
          v-show="page === 'tasks'"
          class="resource-content"
        >
          <Tasks :controller="controller" />
        </div>
        <SoftwareManagement
          v-if="ready"
          v-show="page === 'software-management'"
          :model="software"
          :active="page === 'software-management'"
          :execution="controller"
          @navigation="softwarePage = $event"
          @navigate="emit('navigate', $event)"
        />
        <ToolsPage
          :network-port="networkCheck"
          :execution="controller"
          :active="page === 'device-tools'"
          @task="openTask"
          @navigation="toolsPage = $event"
          v-if="ready"
          v-show="page === 'device-tools'"
          @navigate="contextualNavigate"
        />
        <SystemSettings
          @navigation="systemPage = $event"
          v-if="ready"
          v-show="page === 'system-settings'"
          @home="emit('navigate', 'home')"
        />
        <div v-if="controller && page === 'home'" class="home-content">
          <Home
            :controller="controller"
            :device="device.state.value"
            :draft="assistant?.draft.value ?? ''"
            :connected="assistant?.state.connection === 'connected'"
            @navigate="contextualNavigate"
            @task="openTask"
            @open="openTarget"
            @update:draft="
              (text) => {
                if (assistant) assistant.draft.value = text;
              }
            "
          />
        </div>
        <Teleport :to="target ?? 'body'" :disabled="!target"
          ><Assistant
            v-if="assistant"
            :visible="assistantVisible"
            :controller="assistant"
            :presentation="pageAssistantOpen ? 'context' : 'main'"
            :header-target="headerHost"
            @settings="openConnectionSettings"
            @tasks="emit('navigate', 'tasks')"
        /></Teleport>
        <Settings
          @navigation="settingsPage = $event"
          v-show="!ready || page === 'settings'"
          :host="host"
          :service-port="servicePort"
          :self-service-port="controller?.port ?? selfServicePort"
          :device="device.state.value"
          :fixture="fixture"
          :active="!ready || page === 'settings'"
          :assistant="assistant"
          @assistant="emit('navigate', 'assistant')"
          @tasks="navigateMain('tasks')"
          ><template #development="scope"
            ><slot name="development" /><slot
              name="operation-cards"
              :visible="scope.active"
              :session-id="assistant?.state.selected ?? ''"
              :drafts="assistant?.operationDrafts" /></template
          ><template #user><slot name="user" /></template
          ><template #account-summary
            ><slot name="account-summary">账户与连接</slot></template
          ></Settings
        >
      </div>
      <aside
        v-if="assistant && pageAssistantOpen"
        class="page-assistant-panel"
        aria-label="电脑助手侧栏"
      >
        <PageAssistantPanel
          :context="currentPageContext"
          :controller="assistant"
          :history-open="historyOpen"
          @history="toggleHistory"
          @attach="attachPage"
          @close="togglePageAssistant"
          @main="openMainAssistant"
          ><div ref="pagePanelHost" class="assistant-panel-host"
        /></PageAssistantPanel>
      </aside>
    </div>
    <template #status
      ><span v-if="busy" role="status">正在读取或切换账户…</span
      ><span v-else
        ><template v-if="!fixture"
          >{{
            !ready
              ? "选择使用身份"
              : controller?.interactive
                ? "本机执行服务"
                : "浏览器只读预览"
          }}
          · </template
        >会话保存在本机 · 模型请求发送至所选服务 · 设备状态独立核实</span
      ></template
    >
  </AppShell>
</template>
<style scoped>
.home-content {
  container-type: inline-size;
  height: 100%;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
}
.workspace-page-header {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
}
.assistant-header-host {
  flex: 1;
  min-width: 0;
}
.assistant-header-host[hidden] {
  display: none;
}
.workspace-page-header h1 {
  margin: 0;
  font-size: var(--rss-font-size-md);
  font-weight: 500;
}
.workspace-content {
  --history-width: min(300px, 45vw);
  position: relative;
  display: flex;
  min-width: 0;
  height: 100%;
  min-height: 0;
}
.workspace-content.history-reserved {
  padding-left: calc(var(--history-width) + 16px);
}
.workspace-page-surface {
  flex: 1;
  min-width: 0;
  min-height: 0;
  height: 100%;
}
.conversation-history-panel {
  position: absolute;
  z-index: 50;
  top: 8px;
  bottom: 8px;
  left: 8px;
  width: var(--history-width);
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
  background: var(--rss-color-surface);
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  box-shadow: 0 12px 32px #18223016;
  min-height: 0;
}
.history-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 13px;
  flex: none;
}
.history-actions {
  display: flex;
  gap: 4px;
}
.history-actions button {
  width: 28px;
  height: 28px;
  padding: 0;
}
.page-assistant-panel {
  flex: 0 0 400px;
  width: 400px;
  min-width: 0;
  min-height: 0;
  border-left: 1px solid var(--rss-color-border);
  background: var(--rss-color-bg);
}
@media (max-width: 1000px) {
  .page-assistant-panel {
    position: fixed;
    z-index: 35;
    right: 0;
    top: 56px;
    bottom: 26px;
    width: min(400px, 100vw);
    box-shadow: -12px 0 32px #18223010;
  }
}
.resource-content {
  height: 100%;
  min-width: 0;
  overflow: auto;
  padding: 24px;
}
.assistant-panel-host {
  height: 100%;
  min-height: 0;
}
@media (max-width: 640px) {
  .resource-content {
    padding: 16px;
  }
}
.workspace-breadcrumb {
  display: flex;
  align-items: center;
  overflow: hidden;
  white-space: nowrap;
  gap: 12px;
  min-width: 0;
  color: var(--rss-color-text-muted);
  font-size: 13px;
}
.workspace-breadcrumb button {
  flex: none;
  border: 0;
  background: transparent;
  padding: 5px 0;
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.workspace-breadcrumb [aria-current] {
  color: var(--rss-color-text);
  font-weight: 600;
}
.workspace-assistant-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 0;
  background: transparent;
  width: 30px;
  height: 30px;
  padding: 0;
  justify-content: center;
  border-radius: 6px;
  color: var(--rss-color-text-muted);
}
.workspace-assistant-entry:hover {
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text);
}
.workspace-settings-footer {
  margin-top: 24px;
  padding-top: 16px;
  border-top: 1px solid var(--rss-color-border);
}
.agent-settings-navigation {
  gap: 2px;
  max-height: min(42vh, 324px);
  overflow-y: auto;
  padding-bottom: 12px;
}
.agent-settings-navigation :deep(button) {
  min-height: 32px;
  padding: 8px 10px;
  line-height: 16px;
}
:global(.hidden-account .agent-settings-navigation) {
  display: none;
}
:global(.hidden-account .workspace-settings-footer) {
  margin: 0;
  padding: 0;
  border: 0;
}
@media (max-width: 600px) {
  .workspace-assistant-entry span {
    display: none;
  }
  .workspace-breadcrumb {
    gap: 6px;
    font-size: 11px;
  }
}
</style>
