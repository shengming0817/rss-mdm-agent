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
  UiDialog,
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
import { pageContext } from "./workspace/page-context";
import { nativeAssistant } from "./assistant/native";
import Assistant from "./assistant/Assistant.vue";
import ContextPanel from "./assistant/ContextPanel.vue";
import {
  resourceContext,
  contextCurrent,
  type ResourceContext,
} from "./assistant/resource-context";
import type { BackendTask, SelfServicePort } from "./self-service/types";
import {
  createAssistant,
  type AssistantServices,
} from "./assistant/controller";
import SelfService from "./self-service/SelfService.vue";
import Home from "./home/Home.vue";
import SystemSettings from "./system-settings/SystemSettings.vue";
import SoftwarePreviewPage from "./catalog-preview/SoftwareManagement.vue";
import SoftwareManagement from "./software/SoftwareManagement.vue";
import { createSoftware, softwareSummaryKey } from "./software/controller";
import { nativeDataPort, nativeSoftwarePort } from "./self-service/native";
import ToolsPage from "./catalog-preview/ToolsPage.vue";
import { createDeviceOverview, type DeviceOverviewPort } from "./home/device";
import { nativeDeviceOverview } from "./home/native";
import { createController } from "./self-service/controller";
import { nativePort } from "./self-service/native";
import preview from "./self-service/preview";
import Settings from "./settings/Settings.vue";
import type { ServicePort } from "./settings/native";
import type { HostSettings } from "./settings/controller";
import "./self-service/style.css";
import "./assistant/style.css";
const props = defineProps<{
  ready: boolean;
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
import type { PageNavigation } from "./workspace/page-navigation";
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
const searchEntries = computed(() => [
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
    (module?.sections ?? []).map((e) => ({ ...e, page, group })),
  ),
  ...(softwarePage.value?.searchEntries ?? []).map((e) => ({
    ...e,
    page: "software-management",
    group: "应用",
  })),
  ...(toolsPage.value?.searchEntries ?? []).map((e) => ({
    ...e,
    page: "device-tools",
    group: "工具",
  })),
  {
    page: "system-settings",
    id: "lock",
    title: "屏幕与睡眠",
    group: "系统设置 · 电源",
  },
]);
function searchTarget(entry: { page: string; id: string }) {
  const module =
    entry.page === "software-management"
      ? softwarePage.value
      : entry.page === "device-tools"
        ? toolsPage.value
        : entry.page === "system-settings"
          ? systemPage.value
          : settingsPage.value;
  module?.openSection(entry.id);
  emit("navigate", entry.page);
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
const lastContentPage = ref("home");
const newIdentity = () => crypto.randomUUID();
const software = createSoftware(
  !props.ready || props.fixture ? null : nativeDataPort(),
  !props.ready || props.fixture ? null : nativeSoftwarePort(),
);
provide(softwareSummaryKey, software.summary);
onBeforeUnmount(() => software.dispose());
const controller = props.ready
  ? createController(props.selfServicePort ?? nativePort(), preview)
  : undefined;
const assistant = props.ready
  ? createAssistant(props.assistantServices ?? nativeAssistant(), newIdentity)
  : undefined;
watch(
  () => props.page,
  (id) => {
    if (id !== "settings") settingsExpanded.value = false;
    if (id !== "assistant") lastContentPage.value = id;
    if (["home", "software", "tools", "tasks"].includes(id))
      controller?.navigate(id);
    if (id === "home") void device.refresh();
    if (!historyPinned.value) historyOpen.value = false;
  },
  { immediate: true },
);
const contextOpen = ref(false),
  wide = ref(false),
  chosen = ref(false),
  moving = ref(false);
const candidate = shallowRef<ResourceContext>(),
  target = shallowRef<HTMLElement>();
const panelHost = ref<HTMLElement>();
const pageAssistantOpen = ref(false);
const pagePanelHost = ref<HTMLElement>();
const pageAssistantTrigger = ref<HTMLButtonElement>();
const currentPageContext = computed(() =>
  pageContext(
    props.page,
    moduleTitle.value || (props.page === "assistant" ? "AI 助手" : "工作区"),
    currentModule.value?.path ?? [],
    currentModule.value?.resources ??
      (props.page === "home"
        ? [
            {
              id: "computer",
              title: device.state.value?.attributes.computerName ?? "这台电脑",
              description: "当前设备概览",
              status: device.state.value ? "已读取设备名称" : "设备信息未读取",
              source: "设备概览",
            },
          ]
        : []),
  ),
);
function attachPage() {
  assistant?.stageContext(currentPageContext.value);
}
async function togglePageAssistant() {
  if (!assistant) return;
  const opening = !pageAssistantOpen.value;
  if (opening && props.page === "assistant")
    emit("navigate", lastContentPage.value);
  await relocate(() => {
    pageAssistantOpen.value = opening;
    contextOpen.value = false;
    candidate.value = undefined;
    chosen.value = false;
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
watch([currentPageContext, () => assistant?.state.selected], () => {
  if (pageAssistantOpen.value) attachPage();
});
const headerHost = ref<HTMLElement>();
const contextSelection = ref("");
const panelError = ref(""),
  panelNotice = ref("");
let transition = 0;
let trigger: HTMLElement | null = null;
const assistantVisible = computed(
  () =>
    !moving.value &&
    (props.page === "assistant" ||
      pageAssistantOpen.value ||
      (contextOpen.value && chosen.value)),
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
  else if (contextOpen.value && chosen.value) target.value = panelHost.value;
  moving.value = false;
}
async function askAi(item: BackendTask, source: HTMLElement) {
  const snapshot = controller?.state.snapshot;
  if (!snapshot || !assistant) return;
  const value = resourceContext(item, assistant.state.mode);
  if (
    !value ||
    !contextCurrent(value, snapshot.available, assistant.state.mode)
  )
    return;
  trigger = source;
  await relocate(() => {
    pageAssistantOpen.value = false;
    candidate.value = value;
    contextSelection.value = "";
    chosen.value = false;
    panelError.value = "";
    panelNotice.value = "";
    contextOpen.value = true;
  });
}
async function chooseConversation(id: string) {
  const value = candidate.value,
    version = transition;
  if (!assistant || !value || value.stale) return;
  if (id !== "new" && !assistant.state.sessions.has(id)) {
    panelError.value = "会话不可用，请重新选择。";
    return;
  }
  if (id === "new") assistant.create();
  else await assistant.select(id);
  if (
    version !== transition ||
    !contextOpen.value ||
    candidate.value !== value ||
    value.stale
  )
    return;
  if (id !== "new" && assistant.state.selected !== id) return;
  panelNotice.value =
    assistant.context.value && assistant.context.value.text !== value.text
      ? `已替换待发送资源为“${value.path[2]}”，原问题已保留。`
      : "";
  assistant.stageContext(value);
  await relocate(() => {
    chosen.value = true;
  });
  await nextTick();
  panelHost.value?.querySelector<HTMLTextAreaElement>("textarea")?.focus();
}
async function closeContext() {
  await relocate(() => {
    contextOpen.value = false;
    candidate.value = undefined;
    chosen.value = false;
  });
  if (trigger?.isConnected && trigger.getClientRects().length) trigger.focus();
}
async function mainConversation() {
  await closeContext();
  emit("navigate", "assistant");
}
watch(
  () => props.page,
  () => {
    if (contextOpen.value) void closeContext();
    if (props.page === "assistant" && pageAssistantOpen.value)
      void relocate(() => {
        pageAssistantOpen.value = false;
      });
  },
);
watch(
  () => controller?.state.snapshot,
  (snapshot) => {
    if (!snapshot) return;
    assistant?.validateContexts(
      snapshot.available,
      assistant?.state.mode ?? "live",
    );
    if (
      candidate.value &&
      !contextCurrent(
        candidate.value,
        snapshot.available,
        assistant?.state.mode ?? "live",
      )
    )
      candidate.value = Object.freeze({ ...candidate.value, stale: true });
  },
  { deep: true },
);
let sizing: ResizeObserver | undefined;
const workspaceRoot = ref<HTMLElement>();
const attention = computed(() => assistant?.attention.value ?? 0);
const scrollPositions = new Map<string, number>();
watch(
  () => props.page,
  async (page, previous) => {
    const root = workspaceRoot.value;
    const scroller =
      previous === "home"
        ? root?.querySelector<HTMLElement>(".home-content")
        : root?.closest("main");
    if (previous && scroller) scrollPositions.set(previous, scroller.scrollTop);
    await nextTick();
    if (props.page !== page) return;
    const destination =
      page === "home"
        ? workspaceRoot.value?.querySelector<HTMLElement>(".home-content")
        : workspaceRoot.value?.closest("main");
    if (destination) destination.scrollTop = scrollPositions.get(page) ?? 0;
  },
  { flush: "pre" },
);
onMounted(() => {
  void device.refresh();
  devicePolling = setInterval(() => {
    if (props.page === "home") void device.refresh();
  }, 30000);
  void assistant?.connect();
  if (workspaceRoot.value) {
    const resize = () => {
      const outer =
        workspaceRoot.value!.closest(".workspace-chrome") ??
        workspaceRoot.value!;
      chromeNarrow.value = outer.getBoundingClientRect().width < 900;
      const next = workspaceRoot.value!.getBoundingClientRect().width >= 1200;
      if (wide.value !== next) {
        wide.value = next;
        if (contextOpen.value && chosen.value) void relocate(() => {});
      }
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
  device.dispose();
  transition++;
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
                      : page === "software"
                        ? "软件中心"
                        : page === "tools"
                          ? "工具中心"
                          : page === "help"
                            ? "设备与帮助"
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
        @select="searchTarget"
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
      <div
        class="workspace-page-surface"
        :class="{ 'context-layout': contextOpen && wide }"
      >
        <div
          class="resource-content"
          v-if="controller"
          v-show="
            ![
              'assistant',
              'settings',
              'home',
              'system-settings',
              'software-management',
              'device-tools',
            ].includes(page)
          "
        >
          <SelfService
            v-if="controller"
            v-show="
              ![
                'assistant',
                'settings',
                'home',
                'system-settings',
                'software-management',
                'device-tools',
              ].includes(page)
            "
            :controller="controller"
            :context-key="candidate?.key ?? assistant?.context.value?.key"
            :source="
              assistant?.state.mode === 's1'
                ? 's1'
                : controller.interactive
                  ? 'live'
                  : 'preview'
            "
            @ask-ai="askAi"
          />
        </div>
        <SoftwarePreviewPage
          v-if="ready && fixture"
          v-show="page === 'software-management'"
          @navigation="softwarePage = $event"
          @navigate="emit('navigate', $event)"
        />
        <SoftwareManagement
          v-if="ready && !fixture"
          v-show="page === 'software-management'"
          :model="software"
          :execution="controller"
          @navigation="softwarePage = $event"
          @navigate="emit('navigate', $event)"
        />
        <ToolsPage
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
            :fixture="!!fixture"
            :device="device.state.value"
            :preview="!controller.interactive || !!fixture"
            :draft="assistant?.draft.value ?? ''"
            :connected="assistant?.state.connection === 'connected'"
            @navigate="contextualNavigate"
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
            :presentation="
              pageAssistantOpen || (contextOpen && chosen) ? 'context' : 'main'
            "
            :header-target="headerHost"
            @settings="openConnectionSettings"
            @tasks="emit('navigate', 'tasks')"
        /></Teleport>
        <UiDialog
          :open="true"
          presentation="drawer"
          v-if="contextOpen && !wide && candidate && assistant"
          class="resource-context-drawer"
          title="资源上下文 AI"
          side="right"
          @update:open="closeContext"
        >
          <ContextPanel
            v-model:selection="contextSelection"
            :wide="false"
            :candidate="candidate"
            :controller="assistant"
            :chosen="chosen"
            :notice="panelNotice"
            :error="
              panelError ||
              (candidate.stale && !chosen
                ? '资源信息已变更，请从最新资源详情重新选择。'
                : '')
            "
            @choose="chooseConversation"
            @close="closeContext"
            @main="mainConversation"
            ><div ref="panelHost" class="assistant-panel-host"
          /></ContextPanel>
        </UiDialog>
        <aside
          v-if="contextOpen && wide && candidate && assistant"
          class="resource-context-aside"
          aria-label="资源上下文 AI"
        >
          <ContextPanel
            v-model:selection="contextSelection"
            :wide="true"
            :candidate="candidate"
            :controller="assistant"
            :chosen="chosen"
            :notice="panelNotice"
            :error="
              panelError ||
              (candidate.stale && !chosen
                ? '资源信息已变更，请从最新资源详情重新选择。'
                : '')
            "
            @choose="chooseConversation"
            @close="closeContext"
            @main="mainConversation"
            ><div ref="panelHost" class="assistant-panel-host"
          /></ContextPanel>
        </aside>
        <Settings
          @navigation="settingsPage = $event"
          v-show="!ready || page === 'settings'"
          :host="host"
          :service-port="servicePort"
          :self-service-port="selfServicePort"
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
              :session-id="assistant?.state.selected ?? ''" /></template
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
.context-layout {
  display: flex;
}
.context-layout > .resource-content {
  flex: 1;
}
.assistant-panel-host {
  height: 100%;
  min-height: 0;
}
:global(.resource-context-drawer) {
  --rss-drawer-width: 520px;
  --rss-drawer-padding: 16px;
}
:global(.resource-context-drawer .ui-dialog-body > .context-panel) {
  flex: 1;
  min-height: 0;
}
.resource-context-aside {
  min-width: 0;
  flex: 0 0 var(--rss-inspector-width);
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
