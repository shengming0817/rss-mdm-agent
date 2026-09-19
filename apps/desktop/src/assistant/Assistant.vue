<script setup lang="ts">
import { computed, ref } from "vue";
import { MessageComposer, MessageStream } from "@rss-mdm-agent/ui";
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
import type { CommandView, TimelineItem } from "@rss-mdm-agent/ai-client";
import { permissionPresentation, type AssistantController } from "./controller";
import QuestionCard from "./QuestionCard.vue";
import ExecutionDetails from "./ExecutionDetails.vue";
const props = defineProps<{ controller: AssistantController }>();
const c = props.controller,
  s = c.state,
  view = c.view,
  runtime = c.runtime,
  draft = c.draft;
const clock = c.clock;
const connectionLabel = new Map([
  ["connected", "已连接"],
  ["attached", "已连接"],
  ["detached", "当前会话已分离"],
  ["resync_required", "需要重新读取"],
  ["connecting", "正在连接"],
  ["disconnected", "AI 服务未连接"],
]);
const resumeLabel = new Map([
  ["available", ""],
  ["run-active", "当前模型运行尚未结束，暂不能恢复原上下文"],
  ["not-attached", "请先连接并重新读取当前会话"],
  ["retired", "会话已结束，原上下文不可恢复"],
  ["unsupported", "当前服务不支持恢复原模型上下文"],
  ["unknown", "当前服务尚未确认原上下文恢复能力"],
]);
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
    surfaces = new Map(Object.entries(v.surfaces));
  return v.timeline.map((item) => ({
    ...item,
    command: commands.get(item.key),
    message: messages.get(item.key),
    tool: tools.get(item.key),
    interaction: interactions.get(item.key),
    surface: surfaces.get(item.key),
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
</script>
<template>
  <section class="assistant" aria-label="AI 助手">
    <div class="page-heading">
      <div>
        <span class="eyebrow">ASSISTANT</span>
        <h1>AI 助手</h1>
        <p>讨论问题、跟进工具调用，并独立核对设备执行事实。</p>
      </div>
      <span v-if="s.mode === 's1'" class="mode-label"
        >S1 测试装配 · 无真实执行</span
      >
    </div>
    <div class="assistant-facts" role="status">
      <span>连接：{{ connectionLabel.get(c.sessionConnection.value) }}</span>
      <span>AI：{{ c.busy.value ? "本轮处理中" : "空闲 / 历史可读" }}</span>
      <span
        >设备：{{ s.task ? "来自独立执行服务" : "尚未读取可信执行结果" }}</span
      >
    </div>
    <p v-if="s.error" role="alert">连接失败：{{ s.error }}</p>
    <p v-if="s.listError" role="alert">
      会话列表读取失败：{{ s.listError }}
      <button
        :disabled="s.listing || s.connection !== 'connected'"
        @click="c.list(false)"
      >
        重读会话列表
      </button>
    </p>
    <p v-if="s.createError" role="alert">新建会话失败：{{ s.createError }}</p>
    <p v-if="s.cleanupError" role="alert">
      连接清理未确认：{{ s.cleanupError }}；本地会话已释放。
    </p>
    <div v-if="s.connection !== 'connected'" class="notice">
      <p>连接恢复不会自动重发命令，也不证明原模型或设备执行已停止。</p>
      <button :disabled="s.connection === 'connecting'" @click="c.connect">
        连接 AI 服务
      </button>
    </div>
    <div v-if="c.background.value.length" class="notice">
      后台会话有待回答提问：<button
        v-for="id in c.background.value"
        :key="id"
        @click="c.select(id)"
      >
        {{ id }}
      </button>
    </div>
    <section
      v-for="permission in s.permissions.values()"
      :key="permission.id"
      class="permission-card"
      aria-label="AI 工具权限请求"
    >
      <h2>AI 工具权限请求</h2>
      <p>
        会话 {{ permission.request.sessionId }} ·
        {{ permission.request.toolCall.title }}
      </p>
      <p>本次选择只响应 AI 工具权限请求；不能授予设备执行权限或管理员批准。</p>
      <button
        v-for="option in permission.request.options"
        :key="option.optionId"
        :disabled="!permissionPresentation(option.kind)"
        @click="c.permission(permission.id, option.optionId)"
      >
        <strong>{{
          permissionPresentation(option.kind)?.label ?? "未知权限选项"
        }}</strong>
        <span>{{ permissionPresentation(option.kind)?.scope }}</span>
        <small>提供方说明：{{ option.name }}</small></button
      ><button @click="c.permission(permission.id)">拒绝并关闭</button>
    </section>
    <div class="assistant-workspace">
      <aside class="assistant-sessions">
        <h2>会话</h2>
        <button
          :disabled="s.connection !== 'connected' || s.opening"
          @click="c.create"
        >
          新建会话
        </button>
        <ul>
          <li
            v-for="session in s.sessions.values()"
            :key="session.namespace.sessionId"
          >
            <button
              :aria-current="
                s.selected === session.namespace.sessionId ? 'true' : undefined
              "
              @click="c.select(session.namespace.sessionId)"
            >
              {{ session.namespace.sessionId
              }}<small>{{
                session.status === "retired" ? "已结束" : "会话记录"
              }}</small>
            </button>
          </li>
        </ul>
        <button v-if="s.next" :disabled="s.listing" @click="c.list()">
          加载更多会话
        </button>
      </aside>
      <main class="assistant-conversation">
        <template v-if="view">
          <div class="assistant-actions">
            <button
              :disabled="s.connection !== 'connected' || s.opening"
              @click="c.restore()"
            >
              重新读取历史</button
            ><button
              :disabled="!c.canResume.value || s.opening"
              @click="c.restore(true)"
            >
              恢复原模型上下文</button
            ><button
              :disabled="view.connection !== 'attached'"
              @click="c.detach"
            >
              分离当前会话视图</button
            ><span v-if="!c.canResume.value"
              >{{
                resumeLabel.get(c.resumeReason.value)
              }}；历史记录仍可查看</span
            >
          </div>
          <p v-if="s.errors.get(s.selected)" role="alert">
            操作未确认：{{ s.errors.get(s.selected) }}
          </p>
          <p v-if="s.pending.has(s.selected)" role="status">
            保留原命令等待确认。<button
              :disabled="
                s.sending.has(s.selected) || view.connection !== 'attached'
              "
              @click="c.retry()"
            >
              重试原命令
            </button>
          </p>
          <div class="assistant-timeline" aria-label="会话时间线">
            <article v-for="row in rows" :key="row.kind + ':' + row.key">
              <template
                v-if="
                  row.kind === 'prompt' &&
                  row.command?.command.input.type === 'prompt'
                "
                ><MessageStream
                  :items="[
                    {
                      id: row.key,
                      kind: 'user',
                      text: row.command.command.input.text,
                    },
                  ]"
                />
                <p class="command-state">
                  {{ commandStatus(row.command) }} {{ cancelNote(row.command) }}
                </p>
              </template>
              <template v-else-if="row.message"
                ><MessageStream
                  :items="[
                    { id: row.key, kind: 'assistant', text: row.message.text },
                  ]"
                /><small v-if="!row.message.stable"
                  >生成中 · 尚未持久化</small
                ></template
              >
              <details v-else-if="row.tool" class="tool-call">
                <summary>
                  AI 工具：{{ row.tool.name }} · {{ row.tool.status }}
                </summary>
                <p>工具返回内容仅为对话资料；可信执行结果在右侧独立读取。</p>
                <pre>{{ JSON.stringify(row.tool.arguments, null, 2) }}</pre>
                <pre>{{ row.tool.result?.text }}</pre>
              </details>
              <QuestionCard
                v-else-if="row.interaction && !hasSurface(row)"
                :key="
                  view.namespace.sessionId +
                  ':' +
                  row.key +
                  ':' +
                  view.generation
                "
                :interaction="row.interaction"
                :enabled="questionEnabled(row.key)"
                :now="clock"
                @answer="c.respond(row.key, $event)"
              />
              <RuntimeSurface
                v-else-if="row.surface && runtime && s.a2ui"
                :key="
                  view.namespace.sessionId +
                  ':' +
                  row.key +
                  ':' +
                  view.generation
                "
                :runtime="runtime"
                :session-id="view.namespace.sessionId"
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
                      JSON.stringify(row.surface.messages, null, 2).slice(
                        0,
                        8192,
                      )
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
          <p
            v-for="command in cancellations"
            :key="command.command.commandId"
            role="status"
          >
            AI 取消请求：{{
              command.state === "acknowledged"
                ? "请求已确认"
                : command.state === "invalidated"
                  ? "命令已失效，未确认模型停止"
                  : "已接收"
            }}。{{
              cancelNote(command) || "模型终止情况见原轮状态。"
            }}设备执行状态需独立核对。
          </p>
          <div class="assistant-actions">
            <button
              :disabled="!c.canSteer.value || !draft.trim()"
              @click="c.prompt('steer')"
            >
              引导当前模型运行</button
            ><button :disabled="!c.canCancel.value" @click="c.cancel">
              请求取消 AI 本轮
            </button>
          </div>
          <p v-if="c.busy.value">
            本轮运行期间可以继续编辑，发送后进入下一轮队列。
          </p>
          <MessageComposer
            v-model="draft"
            :disabled="view.sessionStatus === 'retired'"
            :busy="s.sending.has(s.selected)"
            :can-submit="c.canSend.value"
            @submit="c.prompt()"
          />
        </template>
        <p v-else class="empty-state">选择一个会话或新建对话。</p>
      </main>
      <aside class="assistant-execution">
        <h2>设备执行</h2>
        <p>
          使用执行服务的原始请求编号读取授权详情。AI
          文本和工具输出不会写入此面板。
        </p>
        <form @submit.prevent="taskId.trim() && c.taskDetails(taskId.trim())">
          <label>执行请求编号<input v-model="taskId" maxlength="128" /></label
          ><button :disabled="!taskId.trim() || s.taskLoading">
            读取执行详情
          </button>
        </form>
        <p v-if="s.taskError" role="alert">{{ s.taskError }}</p>
        <ExecutionDetails v-if="s.task" :details="s.task" :now="clock" />
      </aside>
    </div>
  </section>
</template>
