<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  MessageComposer,
  MessageStream,
  ModalDrawer,
  UiPopover,
  UiPopoverTrigger,
  UiPopoverContent,
  UiPopoverPortal,
  UiMenu,
  UiMenuTrigger,
  UiMenuContent,
  UiMenuPortal,
  UiMenuItem,
  MoreHorizontal,
  ChevronDown,
  Sparkles,
  MessageSquare,
  ChevronRight,
} from "@rss-mdm-agent/ui";
import { copyText } from "./clipboard";
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
import type { CommandView, TimelineItem } from "@rss-mdm-agent/ai-client";
import {
  operationMessage,
  permissionPresentation,
  type AssistantController,
} from "./controller";
import SessionConnection from "./SessionConnection.vue";
defineEmits<{ settings: []; tasks: [] }>();
import QuestionCard from "./QuestionCard.vue";
import ExecutionDetails from "./ExecutionDetails.vue";
import ExecutionActivity from "./ExecutionActivity.vue";
const diagnosticsOpen = ref(false),
  connectionOpen = ref(false),
  menuOpen = ref(false);
const copyStatus = ref("");
const timeline = ref<HTMLElement>();
const root = ref<HTMLElement>();
const detailsHost = ref<HTMLElement>();
const openedOperation = ref("");
const wideDetails = ref(false);
let detailTrigger: HTMLElement | undefined;
let sizing: ResizeObserver | undefined;
onMounted(() => {
  if (!root.value) return;
  sizing = new ResizeObserver((entries) => {
    wideDetails.value = (entries[0]?.contentRect.width ?? 0) >= 1200;
  });
  wideDetails.value = root.value.getBoundingClientRect().width >= 1200;
  sizing.observe(root.value);
});
onBeforeUnmount(() => sizing?.disconnect());
async function openDetails(id: string) {
  detailTrigger = [
    ...(timeline.value?.querySelectorAll<HTMLElement>(
      '[data-action="execution-details"]',
    ) ?? []),
  ].find((el) => el.getAttribute("data-operation-id") === id);
  openedOperation.value = id;
  await nextTick();
  if (wideDetails.value)
    detailsHost.value?.querySelector<HTMLElement>("button")?.focus();
}
async function closeDetails() {
  openedOperation.value = "";
  await nextTick();
  if (detailTrigger?.isConnected) detailTrigger.focus();
}
const following = ref(true);
function trackScroll() {
  const el = timeline.value;
  if (el)
    following.value = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
}
function scrollEnd() {
  if (!props.visible) return;
  const el = timeline.value;
  if (el) el.scrollTop = el.scrollHeight;
  following.value = true;
}
const props = defineProps<{
  controller: AssistantController;
  visible: boolean;
  portalTarget?: HTMLElement;
  headerTarget?: HTMLElement;
  presentation: "main" | "context";
}>();
const c = props.controller,
  s = c.state,
  view = c.view,
  runtime = c.runtime,
  draft = c.draft;
const clock = c.clock;
const retryLabel = new Map([
  ["same_command", "仅可核对并重试原命令，勿新建重复命令"],
  ["reconcile_first", "先恢复历史并核对，再决定后续操作"],
  ["never", "此命令不可重试"],
]);
const taskId = ref("");
const rows = computed(() => {
  const v = view.value;
  if (!v) return [];
  const commands = new Map(Object.entries(v.commands)),
    messages = new Map(Object.entries(v.messages)),
    tools = new Map(Object.entries(v.tools)),
    interactions = new Map(Object.entries(v.interactions)),
    surfaces = new Map(Object.entries(v.surfaces)),
    deliveries = new Map(Object.entries(v.deliveries));
  return v.timeline.map((item) => ({
    ...item,
    command: commands.get(item.key),
    message: messages.get(item.key),
    tool: tools.get(item.key),
    interaction: interactions.get(item.key),
    surface: surfaces.get(item.key),
    delivery: deliveries.get(item.key),
    surfaceInteraction: interactions.get(
      surfaces.get(item.key)?.interactionId ?? "",
    ),
  }));
});
function commandStatus(command: CommandView) {
  if (command.state === "cancelled") return "已取消排队，未发送给模型";
  if (command.state === "acknowledged")
    return "控制命令已确认；模型本轮状态见原始请求";
  if (command.state === "invalidated") return "本地命令失效；未确认模型终止";
  if (command.state === "terminal")
    return `模型本轮结束：${command.outcome ?? "未知"}；设备效果需独立查询`;
  if (command.state === "reconciliation_required")
    return "AI 派发结果待核对，请勿重新发送同一任务";
  if (command.state === "running") return "模型正在处理";
  if (command.state === "dispatching") return "AI 正在派发";
  return "AI 命令已接收 / 排队中";
}
function cancelNote(command: CommandView) {
  if (command.acknowledgement?.type === "queued_cancelled")
    return "已取消排队，未发送给模型。";
  switch (
    command.acknowledgement?.type === "cancel"
      ? command.acknowledgement.confirmation
      : command.cancelDispatched
  ) {
    case "request_only":
      return command.state === "terminal"
        ? "取消请求已发出；模型终止事实已记录。"
        : "取消已发送给模型；等待模型终止事实。";
    case "already_terminal":
      return "取消请求返回模型已终止；设备效果仍需独立核对。";
    case "unsupported":
      return "当前模型不支持取消；原运行状态保持不变。";
    default:
      return "";
  }
}
function questionEnabled(id: string) {
  return c.answerable(id);
}
function hasSurface(item: TimelineItem) {
  return Object.values(view.value?.surfaces ?? {}).some(
    (surface) => surface.interactionId === item.key,
  );
}
const diagnostics = computed(() =>
  Object.values(view.value?.commands ?? {}).filter(
    (command) => command.failure,
  ),
);
const cancellations = computed(() =>
  Object.values(view.value?.commands ?? {}).filter(
    (command) => command.command.input.type === "cancel",
  ),
);

const empty = computed(
  () =>
    !rows.value.length &&
    !s.opening &&
    !s.pending.has(s.selected) &&
    !permissions.value.length &&
    (!s.selected ||
      (!!view.value &&
        view.value.connection === "attached" &&
        !s.errors.get(s.selected))),
);
const recent = computed(() =>
  c.sessions.value.find((row) => row.status !== "retired"),
);
async function suggest(value: string) {
  if (draft.value.trim()) return;
  draft.value = value;
  await nextTick();
  root.value?.querySelector<HTMLTextAreaElement>("textarea")?.focus();
}
const title = computed(() => s.sessions.get(s.selected)?.title ?? "新对话");
const connectionName = computed(
  () =>
    s.connections.find(
      (row) => row.connectionId === c.selectedConnectionId.value,
    )?.name ?? "选择连接",
);
const permissions = computed(() =>
  [...s.permissions.values()].filter(
    (item) => item.request.sessionId === s.selected,
  ),
);
const connectionPanel = ref<HTMLElement>();
async function showContextChoices() {
  connectionOpen.value = true;
  await nextTick();
  connectionPanel.value?.querySelector("select")?.focus();
}
const notice = computed(() => {
  if (s.connection === "connecting")
    return { text: "正在连接 AI 服务…", action: "connect" };
  const error =
    s.connection !== "connected"
      ? s.error
      : s.errors.get(s.selected) ||
        s.createError ||
        (s.preferenceSave?.sessionId === s.selected && s.preferenceSave.failed
          ? "preference_not_saved"
          : "");
  if (
    [
      "authentication_required",
      "connection_required",
      "unsupported_capability",
    ].includes(error ?? "")
  )
    return { text: operationMessage(error!), action: "settings" };
  if (error === "context_unavailable")
    return {
      text: operationMessage(error),
      action: s.connection === "connected" ? "context" : "connect",
    };
  if (s.connection !== "connected")
    return {
      text: error ? operationMessage(error) : "AI 服务未连接，草稿已保留。",
      action: "connect",
    };
  if (s.pending.has(s.selected))
    return {
      text: "上条消息是否已接收仍待确认，草稿已保留。",
      action: "retry",
    };
  if (error === "preference_not_saved")
    return { text: operationMessage(error), action: "preference" };
  if (error)
    return {
      text: operationMessage(error),
      action: s.createError ? "first" : "restore",
    };
  if (!c.connectionReady.value)
    return {
      text: "配置并测试 AI 连接后即可发送，草稿会保留。",
      action: "settings",
    };
  if (view.value && view.value.connection !== "attached")
    return { text: "连接已中断，重新读取会话后继续。", action: "restore" };
  return undefined;
});
watch(
  () => [
    s.selected,
    view.value?.cursor,
    permissions.value.map((permission) => permission.id).join("\0"),
    Object.values(view.value?.messages ?? {})
      .map((row) => row.text)
      .join(""),
  ],
  async (next, previous) => {
    if (props.visible && following.value && next[0] === previous?.[0]) {
      await nextTick();
      if (props.visible && following.value && next[0] === s.selected)
        scrollEnd();
    }
  },
  { flush: "post" },
);
type Reading = {
  following: boolean;
  top: number;
  anchor: string | null;
  offset: number;
};
const reading = new Map<string, Reading>();
function remember(id = s.selected) {
  const el = timeline.value;
  if (!el) return;
  const top = el.getBoundingClientRect().top;
  const anchor = [...el.querySelectorAll<HTMLElement>(":scope > article")].find(
    (row) => row.getBoundingClientRect().bottom > top,
  );
  reading.set(id, {
    following: following.value,
    top: el.scrollTop,
    anchor: anchor?.getAttribute("data-message-key") ?? null,
    offset: (anchor?.getBoundingClientRect().top ?? top) - top,
  });
}
function restoreReading() {
  const el = timeline.value,
    saved = reading.get(s.selected);
  if (!el || !props.visible) return;
  if (!saved || saved.following) {
    scrollEnd();
    return;
  }
  following.value = false;
  const anchor = [...el.querySelectorAll<HTMLElement>(":scope > article")].find(
    (row) => row.getAttribute("data-message-key") === saved.anchor,
  );
  el.scrollTop = anchor
    ? el.scrollTop +
      anchor.getBoundingClientRect().top -
      el.getBoundingClientRect().top -
      saved.offset
    : saved.top;
}
watch(
  () => s.selected,
  async (_, previous) => {
    if (props.visible) remember(previous);
    openedOperation.value = "";
    diagnosticsOpen.value = connectionOpen.value = menuOpen.value = false;
    copyStatus.value = "";
    await nextTick();
    restoreReading();
  },
  { flush: "pre" },
);
watch(
  () => props.visible,
  async (visible) => {
    if (!visible) {
      remember();
      openedOperation.value = "";
      diagnosticsOpen.value = connectionOpen.value = menuOpen.value = false;
      copyStatus.value = "";
    } else {
      await nextTick();
      restoreReading();
    }
  },
  { flush: "pre" },
);
onBeforeUnmount(() => reading.clear());
async function copy(text: string) {
  const selected = s.selected;
  const copied = await copyText(text);
  if (selected === s.selected && props.visible)
    copyStatus.value = copied ? "已复制" : "复制不可用，请选中文本手动复制。";
}
</script>
<template>
  <section
    ref="root"
    v-show="visible"
    class="assistant"
    :class="{
      'assistant-context': presentation === 'context',
    }"
    aria-label="AI 助手"
  >
    <section
      class="assistant-conversation"
      :class="{ 'assistant-start': empty }"
      aria-label="当前对话"
    >
      <Teleport
        :to="headerTarget ?? 'body'"
        :disabled="presentation === 'context' || !headerTarget"
      >
        <header v-show="visible" class="conversation-header">
          <h1 v-if="presentation === 'main'" tabindex="-1">{{ title }}</h1>
          <span v-else class="context-session">{{ title }}</span>
          <UiPopover v-model:open="connectionOpen">
            <UiPopoverTrigger class="connection-trigger"
              >{{ connectionName }} <ChevronDown :size="14" aria-hidden="true"
            /></UiPopoverTrigger>
            <UiPopoverPortal :to="portalTarget"
              ><UiPopoverContent
                class="rss-ui rss-popover connection-menu"
                :side-offset="8"
                align="end"
                aria-label="AI 连接选择"
              >
                <div ref="connectionPanel">
                  <SessionConnection :controller="c" /><button
                    @click="
                      connectionOpen = false;
                      $emit('settings');
                    "
                  >
                    管理 AI 连接
                  </button>
                </div>
              </UiPopoverContent></UiPopoverPortal
            >
          </UiPopover>
          <!-- The native drawer owns dialog focus; its launcher must not trap it. -->
          <UiMenu v-model:open="menuOpen" :modal="false">
            <UiMenuTrigger
              class="icon-button conversation-menu"
              aria-label="更多"
              ><MoreHorizontal :size="20" aria-hidden="true"
            /></UiMenuTrigger>
            <UiMenuPortal :to="portalTarget"
              ><UiMenuContent
                class="rss-ui rss-menu"
                @close-auto-focus="
                  (event) => {
                    if (diagnosticsOpen) event.preventDefault();
                  }
                "
                :side-offset="8"
                align="end"
              >
                <UiMenuItem @select="diagnosticsOpen = true"
                  >会话详情与诊断</UiMenuItem
                >
                <UiMenuItem @select="$emit('settings')">设置</UiMenuItem>
              </UiMenuContent></UiMenuPortal
            >
          </UiMenu>
        </header>
      </Teleport>
      <div v-if="c.background.value.length" class="notice">
        其他对话需要回应：<button
          v-for="id in c.background.value"
          :key="id"
          @click="c.select(id)"
        >
          {{ s.sessions.get(id)?.title ?? "新对话" }}
        </button>
      </div>
      <div v-if="empty" class="assistant-welcome">
        <span class="welcome-mark"
          ><Sparkles :size="28" aria-hidden="true"
        /></span>
        <h2>
          {{
            presentation === "main" ? "今天，想完成什么？" : "想了解这个资源？"
          }}
        </h2>
        <p>
          {{
            presentation === "main"
              ? "讨论想法、解决问题，也可以了解这台设备上的任务。"
              : "解释用途、核对版本与要求。资源信息由你确认后发送。"
          }}
        </p>
      </div>
      <div
        v-show="!empty"
        ref="timeline"
        class="assistant-timeline"
        role="log"
        aria-live="polite"
        aria-relevant="additions text"
        aria-label="会话时间线"
        @scroll="trackScroll"
      >
        <article
          v-for="row in rows"
          :key="s.selected + ':' + row.kind + ':' + row.key"
          :data-message-key="row.kind + ':' + row.key"
        >
          <template
            v-if="
              row.kind === 'prompt' &&
              row.command?.command.input.type === 'prompt'
            "
            ><MessageStream
              :announce="false"
              :items="[
                {
                  id: row.key,
                  kind: 'user',
                  text: row.command.command.input.text,
                },
              ]"
            />
            <p v-if="row.command.state !== 'terminal'" class="command-state">
              {{ commandStatus(row.command) }} {{ cancelNote(row.command) }}
            </p>
          </template>
          <template v-else-if="row.message"
            ><MessageStream
              :announce="false"
              :items="[
                {
                  id: row.key,
                  kind: 'assistant',
                  text: row.message.text,
                  stable: row.message.stable,
                },
              ]"
              @copy="copy"
            /><small v-if="!row.message.stable"
              >生成中 · 尚未持久化</small
            ></template
          >
          <details v-else-if="row.tool" class="tool-call">
            <summary>
              AI 工具：{{ row.tool.name }} · {{ row.tool.status }}
            </summary>
            <p>工具返回内容由模型服务提供。</p>
            <pre>{{ JSON.stringify(row.tool.arguments, null, 2) }}</pre>
            <pre>{{ row.tool.result?.text }}</pre>
          </details>
          <ExecutionActivity
            v-else-if="
              row.delivery && row.delivery.proposal.name === 'execution_execute'
            "
            :controller="c"
            :operation-id="row.key"
            :recorded="row.delivery.recorded"
            :visible="visible"
            :now="clock"
            :details-open="openedOperation === row.key"
            :details-host="detailsHost"
            @open="openDetails"
            @close="openedOperation === row.key && closeDetails()"
            @tasks="$emit('tasks')"
          />
          <QuestionCard
            v-else-if="row.interaction && !hasSurface(row)"
            :key="
              view!.namespace.sessionId + ':' + row.key + ':' + view!.generation
            "
            :interaction="row.interaction"
            :enabled="questionEnabled(row.key)"
            :now="clock"
            @answer="c.respond(row.key, $event)"
          />
          <RuntimeSurface
            v-else-if="row.surface && runtime && s.a2ui"
            :key="
              view!.namespace.sessionId + ':' + row.key + ':' + view!.generation
            "
            :runtime="runtime"
            :session-id="view!.namespace.sessionId"
            :instance-id="row.key"
          >
            <template #fallback>
              <QuestionCard
                v-if="row.surfaceInteraction"
                :interaction="row.surfaceInteraction"
                :enabled="false"
                :read-only="true"
                :now="clock"
              />
              <details>
                <summary>只读卡片内容（最多 8192 字符）</summary>
                <pre>{{
                  JSON.stringify(row.surface.messages, null, 2).slice(0, 8192)
                }}</pre>
              </details>
            </template>
          </RuntimeSurface>
          <div v-else-if="row.surface">
            <p>当前连接不支持交互卡片；普通文本与历史记录仍可读取。</p>
            <QuestionCard
              v-if="row.surfaceInteraction"
              :interaction="row.surfaceInteraction"
              :enabled="false"
              :read-only="true"
              :now="clock"
            />
          </div>
        </article>

        <section
          v-for="permission in permissions"
          :key="permission.id"
          class="permission-card"
          aria-label="AI 工具权限请求"
        >
          <h2>{{ permission.request.toolCall.title }}</h2>
          <p>
            允许 AI 发起本次请求；设备操作仍需通过设备授权与必要的动作确认。
          </p>
          <details v-if="permission.request.toolCall.rawInput">
            <summary>AI 提议内容（尚未执行）</summary>
            <pre>{{
              JSON.stringify(permission.request.toolCall.rawInput, null, 2)
            }}</pre>
          </details>
          <button
            v-for="option in permission.request.options"
            :key="option.optionId"
            :disabled="!permissionPresentation(option.kind)"
            @click="c.permission(permission.id, option.optionId)"
          >
            <strong>{{
              permissionPresentation(option.kind)?.label ?? "未知权限选项"
            }}</strong
            ><span>{{ permissionPresentation(option.kind)?.scope }}</span
            ><small>提供方说明：{{ option.name }}</small>
          </button>
          <button @click="c.permission(permission.id)">关闭请求</button>
        </section>
      </div>
      <div class="conversation-input">
        <section
          v-if="c.context.value"
          class="resource-context"
          aria-label="待发送资源上下文"
        >
          <div class="resource-context-heading">
            <strong>待发送资源 · {{ c.context.value.path[2] }}</strong
            ><button type="button" @click="c.removeContext()">
              移除上下文
            </button>
          </div>
          <details>
            <summary>核对将发送的信息</summary>
            <pre>{{ c.context.value.text }}</pre>
          </details>
          <p>仅发送所选目录信息，不包含表单输入或设备历史。</p>
        </section>
        <p v-if="c.compositionError.value" role="alert" class="error">
          {{ c.compositionError.value }}
        </p>
        <p v-if="copyStatus" class="copy-status" role="status">
          {{ copyStatus }}
        </p>
        <button v-if="!following" @click="scrollEnd">回到最新消息</button>
        <div v-if="notice" class="conversation-notice" role="status">
          <span>{{ notice.text }}</span>
          <button
            v-if="notice.action === 'connect'"
            :disabled="s.connection === 'connecting'"
            @click="c.connect"
          >
            重新连接
          </button>
          <button
            v-else-if="notice.action === 'settings'"
            @click="$emit('settings')"
          >
            前往连接设置
          </button>
          <button
            v-else-if="notice.action === 'context'"
            @click="showContextChoices"
          >
            选择连接与新上下文
          </button>
          <button
            v-else-if="notice.action === 'first'"
            :disabled="s.opening"
            @click="c.prompt()"
          >
            重试首次发送
          </button>
          <button
            v-else-if="notice.action === 'retry'"
            :disabled="
              s.sending.has(s.selected) || view?.connection !== 'attached'
            "
            @click="c.retry()"
          >
            重试原消息
          </button>
          <button
            v-else-if="notice.action === 'preference'"
            :disabled="s.preferenceSave?.saving"
            @click="c.retryPreference()"
          >
            重试保存当前选择
          </button>
          <button v-else :disabled="s.opening" @click="c.restore()">
            重新读取
          </button>
        </div>
        <button
          v-if="c.canSteer.value"
          :disabled="!c.promptPreview.value"
          @click="c.prompt('steer')"
        >
          调整当前任务
        </button>
        <MessageComposer
          :placeholder="
            presentation === 'context' && c.context.value
              ? '输入关于此资源的问题…'
              : empty
                ? '向 RSS 提问，或描述你希望完成的事…'
                : '继续提问…'
          "
          v-model="draft"
          :disabled="view?.sessionStatus === 'retired'"
          :busy="s.sending.has(s.selected) || s.opening"
          :can-submit="c.canSend.value && !!c.promptPreview.value"
          :submit-label="c.busy.value ? '排队发送' : '发送'"
          :can-cancel="c.canCancel.value"
          @submit="c.prompt()"
          @cancel="c.cancel"
        >
          <template #tools
            ><span class="status-badge"
              >{{ connectionName }} ·
              {{ c.connectionReady.value ? "连接可用" : "待配置或验证" }}</span
            ></template
          >
        </MessageComposer>
      </div>
      <div v-if="empty" class="assistant-start-followup">
        <div class="start-suggestions" aria-label="提问引导">
          <button
            type="button"
            data-action="suggest-prompt"
            :disabled="!!draft.trim()"
            @click="
              suggest(
                presentation === 'main'
                  ? '帮我整理一个想法：'
                  : '解释这个资源的用途与限制。',
              )
            "
          >
            <MessageSquare :size="16" aria-hidden="true" />{{
              presentation === "main" ? "整理一个想法" : "解释资源用途"
            }}
          </button>
          <button
            type="button"
            :disabled="!!draft.trim()"
            @click="
              suggest(
                presentation === 'main'
                  ? '帮我理解软件与工具的使用要求。'
                  : '核对这个资源的版本与运行身份。',
              )
            "
          >
            {{ presentation === "main" ? "理解使用要求" : "核对版本与身份" }}
          </button>
        </div>
        <p class="suggestion-note">
          {{
            draft.trim()
              ? "已有草稿，请编辑后显式发送。"
              : "引导仅填入草稿，由你决定是否发送。"
          }}
        </p>
        <button
          v-if="presentation === 'main' && recent"
          class="resume-conversation"
          @click="c.select(recent.namespace.sessionId)"
        >
          <span
            ><small>继续上次的工作</small
            ><strong>{{ recent.title }}</strong></span
          ><ChevronRight :size="18" aria-hidden="true" />
        </button>
      </div>
    </section>
    <aside
      v-if="
        visible && openedOperation && wideDetails && presentation === 'main'
      "
      class="execution-inspector"
      aria-label="设备操作详情"
    >
      <header>
        <h2>设备操作详情</h2>
        <button
          type="button"
          class="icon-button"
          aria-label="关闭设备操作详情"
          @click="closeDetails"
        >
          ×
        </button>
      </header>
      <div ref="detailsHost" />
    </aside>
    <ModalDrawer
      v-else-if="visible && openedOperation"
      label="设备操作详情"
      side="right"
      @close="closeDetails"
      ><div ref="detailsHost"
    /></ModalDrawer>
    <ModalDrawer
      v-if="visible && diagnosticsOpen"
      label="会话详情与诊断"
      side="right"
      @close="diagnosticsOpen = false"
    >
      <p>会话编号：{{ s.selected || "尚未创建" }}</p>
      <p v-if="s.mode === 's1'">S1 测试装配 · 无真实执行</p>
      <p>连接：{{ c.sessionConnection.value }}</p>
      <p v-if="s.cleanupError">连接清理未确认：{{ s.cleanupError }}</p>
      <div v-if="view" class="assistant-actions">
        <button
          :disabled="s.connection !== 'connected' || s.opening"
          @click="c.restore()"
        >
          重新读取历史
        </button>
        <button
          :disabled="!c.canResume.value || s.opening"
          @click="c.restore(true)"
        >
          恢复原模型上下文
        </button>
        <button :disabled="view.connection !== 'attached'" @click="c.detach">
          分离当前会话视图
        </button>
      </div>
      <section
        v-if="diagnostics.length"
        class="command-failures"
        aria-label="AI 命令诊断"
      >
        <h3>AI 命令诊断</h3>
        <template
          v-for="command in diagnostics"
          :key="command.command.commandId"
        >
          <p v-if="command.failure" role="status">
            {{
              command.command.input.type === "prompt"
                ? "消息"
                : command.command.input.type === "cancel"
                  ? "取消"
                  : "回答"
            }}
            {{ command.command.commandId }}：{{ command.failure.code }}。{{
              retryLabel.get(command.failure.retry)
            }}；该诊断不证明模型已终止。
          </p>
        </template>
      </section>

      <p v-for="command in cancellations" :key="command.command.commandId">
        {{ cancelNote(command) }}
      </p>
      <details>
        <summary>按执行编号查询</summary>
        <form @submit.prevent="taskId.trim() && c.taskDetails(taskId.trim())">
          <label>执行请求编号<input v-model="taskId" maxlength="128" /></label
          ><button :disabled="!taskId.trim() || s.taskLoading">
            读取执行详情
          </button>
        </form>
        <p v-if="s.taskError" role="alert">{{ s.taskError }}</p>
        <ExecutionDetails v-if="s.task" :details="s.task" :now="clock" />
      </details>
    </ModalDrawer>
  </section>
</template>
