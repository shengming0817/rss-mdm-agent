<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import {
  requestRef,
  taskKey,
  taskTitle,
  taskPhase,
  type Controller,
} from "./controller";
import ExecutionDetails from "../assistant/ExecutionDetails.vue";
import PolicyPreparation from "../self-service/PolicyPreparation.vue";
import type {
  TaskAction,
  TaskQuery,
  BackendRequest,
} from "../self-service/types";
const emit = defineEmits<{ back: [] }>();
const props = defineProps<{ controller: Controller }>();
const c = props.controller,
  s = c.state;
const selected = computed(() => s.selected);
const preparation = ref<BackendRequest>(),
  preparationBusy = ref(false),
  preparationError = ref("");
let preparationEpoch = 0;
watch(
  () => selected.value && taskKey(selected.value.reference),
  () => {
    preparationEpoch++;
    preparation.value = undefined;
    preparationError.value = "";
    preparationBusy.value = false;
  },
);
async function readPreparation() {
  const selected = s.selected;
  if (
    !c.port ||
    !selected ||
    selected.reference.kind !== "request" ||
    preparationBusy.value
  )
    return;
  const epoch = preparationEpoch;
  preparationBusy.value = true;
  try {
    const result = await c.port.policy_status({
      requestId: selected.reference.request,
    });
    if (epoch !== preparationEpoch) return;
    if (result.kind === "preparation") preparation.value = result.value;
    else preparationError.value = "原请求已进入执行或远程阶段，请刷新任务事实";
  } catch {
    if (epoch === preparationEpoch)
      preparationError.value = "原准备请求读取未确认";
  } finally {
    if (epoch === preparationEpoch) preparationBusy.value = false;
  }
}
async function confirmPolicy(request: string) {
  if (!c.port || preparationBusy.value || !s.selected?.humanConfirmation)
    return;
  const epoch = preparationEpoch;
  preparationBusy.value = true;
  try {
    await c.port.policy_confirm({ requestId: request });
    if (epoch === preparationEpoch) {
      preparation.value = undefined;
      await c.openTask(requestRef(request));
    }
  } catch {
    if (epoch === preparationEpoch)
      preparationError.value = "确认回执未知，请查询原请求，勿重复确认";
  } finally {
    if (epoch === preparationEpoch) preparationBusy.value = false;
  }
}

const list = ref<HTMLElement>();
const reasons: Record<string, string> = {
  "managed-background-task": "受管后台任务，普通用户不能控制",
  "owner-stage-is-not-interruptible": "当前阶段不支持中断",
  "runner-phase-cannot-be-paused": "执行及校验阶段不支持暂停",
  "native-source-has-no-range-resume": "该原生源准备不支持暂停续传",
  "consumer-paused-shared-producer-active":
    "本任务已暂停；共享材料仍在为其他任务下载",
};
const labels: Record<TaskAction, string> = {
  retry: "明确创建新尝试",
  cancel: "请求取消",
  pause: "暂停本任务下载",
  resume: "恢复本任务下载",
};
async function open(key: string) {
  const item = s.items.find((t) => taskKey(t.reference) === key);
  if (!item) return;
  s.listFocus = key;
  s.listScroll = list.value?.scrollTop ?? 0;
  if ((await c.openTask(item.reference)) === "ready") {
    await nextTick();
    document.querySelector<HTMLElement>(".task-center-detail h2")?.focus();
  }
}
async function back() {
  emit("back");
  s.selected = null;
  s.node = "";
  await nextTick();
  if (list.value) {
    list.value.scrollTop = s.listScroll;
    const button = [
      ...list.value.querySelectorAll<HTMLButtonElement>("button[data-task]"),
    ].find((e) => e.dataset.task === s.listFocus);
    (button ?? list.value.querySelector<HTMLElement>("h2"))?.focus();
  }
}
function filter(
  event: Event,
  key: "source" | "taskKind" | "status" | "policy" | "subject",
) {
  const value = (event.target as HTMLInputElement).value;
  void c.setQuery({ ...s.query, [key]: value || null });
}
function dateInput(value: number | null) {
  if (value == null) return "";
  const date = new Date(value);
  return new Date(value - date.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}
function time(event: Event, key: "from" | "until") {
  const value = (event.target as HTMLInputElement).value;
  void c.setQuery({ ...s.query, [key]: value ? Date.parse(value) : null });
}
</script>
<template>
  <section class="task-center" aria-label="任务中心">
    <header>
      <h1>任务中心</h1>
      <button :disabled="s.loading" @click="c.refresh()">刷新原记录</button>
    </header>
    <nav aria-label="任务视图">
      <button
        v-for="tab in [
          ['active', '正在进行'],
          ['history', '历史记录'],
          ['management', '管理活动'],
        ] as const"
        :key="tab[0]"
        :aria-current="s.query.view === tab[0] ? 'page' : undefined"
        @click="
          s.selected = null;
          c.setQuery({ ...s.query, view: tab[0] });
        "
      >
        {{ tab[1] }}
      </button>
    </nav>
    <form class="task-filters" @submit.prevent>
      <label
        >来源<select
          :value="s.query.source ?? ''"
          @change="filter($event, 'source')"
        >
          <option value="">全部</option>
          <option value="human">人工</option>
          <option value="ai">AI</option>
          <option value="policy">后台 Policy</option>
        </select></label
      >
      <label
        >类型<select
          :value="s.query.taskKind ?? ''"
          @change="filter($event, 'taskKind')"
        >
          <option value="">全部</option>
          <option value="policy">策略</option>
          <option value="query">按需采集</option>
          <option value="software">软件操作</option>
          <option value="sampling">周期采集</option>
          <option value="management">管理活动</option>
        </select></label
      >
      <label
        >状态<input
          :value="s.query.status ?? ''"
          placeholder="原状态"
          @change="filter($event, 'status')"
      /></label>
      <label
        >主体<input
          :value="s.query.subject ?? ''"
          @change="filter($event, 'subject')"
      /></label>
      <label
        >策略 ID<input
          :value="s.query.policy ?? ''"
          @change="filter($event, 'policy')"
      /></label>
      <label
        >开始时间<input
          type="datetime-local"
          :value="dateInput(s.query.from)"
          @change="time($event, 'from')" /></label
      ><label
        >结束时间<input
          type="datetime-local"
          :value="dateInput(s.query.until)"
          @change="time($event, 'until')"
      /></label>
    </form>
    <p v-if="s.error" role="alert">{{ s.error }}</p>
    <p v-if="s.uncertain" role="status">
      控制或提交结果未知，请查询原请求。<button @click="c.reconcileControl()">
        查询原控制回执
      </button>
    </p>
    <section v-if="s.query.view === 'management'" aria-label="同步覆盖">
      <p>
        {{
          s.remoteAvailable ? "本次在线读取成功" : "离线缓存或当前同步未确认"
        }}
      </p>
      <p>
        最近同步：{{
          s.lastSync == null
            ? "尚未同步 / 当前不可确认"
            : new Date(s.lastSync * 1000).toLocaleString()
        }}
        · {{ s.complete ? "本次来源覆盖完整" : "部分或离线历史，不能视为全量" }}
      </p>
      <p v-for="coverage in s.coverage" :key="coverage.source">
        {{ coverage.source }}：{{ coverage.complete ? "已覆盖" : "未完整覆盖" }}
        {{ coverage.reasons.join(" · ") }}
      </p>
      <p v-if="s.retainedFrom != null">
        服务保留起点：{{ new Date(s.retainedFrom * 1000).toLocaleString() }}
      </p>
    </section>
    <div class="task-center-layout" :class="{ 'has-detail': !!selected }">
      <section ref="list" class="task-center-list" aria-label="任务列表">
        <h2 tabindex="-1">原请求与活动</h2>
        <p v-if="s.loading">正在读取…</p>
        <p v-else-if="!s.items.length">
          {{
            s.next
              ? "本页无可见记录，仍可继续读取"
              : s.loaded
                ? "没有匹配的保留记录"
                : "尚未读取"
          }}
        </p>
        <button
          v-for="item in s.items"
          :key="taskKey(item.reference)"
          :data-task="taskKey(item.reference)"
          @click="open(taskKey(item.reference))"
        >
          <strong>{{ taskTitle(item) }}</strong
          ><span
            >{{ taskPhase[item.stage] ?? item.stage }} · {{ item.source }}</span
          ><time v-if="item.recordedAt != null">{{
            new Date(item.recordedAt).toLocaleString()
          }}</time>
        </button>
        <button v-if="s.next" :disabled="s.loading" @click="c.load(true)">
          继续读取
        </button>
      </section>
      <section v-if="selected" class="task-center-detail" aria-label="任务详情">
        <button @click="back">返回任务列表</button>
        <h2 tabindex="-1">{{ taskTitle(selected) }}</h2>
        <p>原引用：{{ JSON.stringify(selected.reference) }}</p>
        <p v-if="selected.policy">
          策略：{{ selected.policy.id }} · 版本 {{ selected.policy.revision }} ·
          修订 {{ selected.policyRevision }}
        </p>
        <p v-if="selected.retryOf">
          本次重新准入关联原请求 {{ selected.retryOf }}
        </p>
        <ExecutionDetails
          v-if="selected.facts.kind === 'execution'"
          :details="selected.facts.value"
          :now="Date.now()"
        />
        <template v-else-if="selected.facts.kind === 'preparation'"
          ><p>
            准备状态：{{ selected.stage }} · revision
            {{ selected.facts.value.revision }}
          </p>
          <template v-if="selected.facts.value.action.kind === 'policy'"
            ><p>输入摘要：{{ selected.parameterSummary }}</p>
            <button :disabled="preparationBusy" @click="readPreparation">
              查看原准备请求并处理
            </button>
            <p v-if="preparationError" role="alert">{{ preparationError }}</p>
            <PolicyPreparation
              v-if="preparation"
              :record="preparation"
              :disabled="preparationBusy || s.busy"
              :confirm-disabled="!selected.humanConfirmation"
              @query="readPreparation"
              @confirm="confirmPolicy"
              @cancel="c.cancel" /></template
          ><PolicyPreparation
            v-else-if="selected.facts.value.action.kind === 'softwareRead'"
            :record="selected.facts.value"
          />
          <p v-if="selected.facts.value.failure">
            {{ selected.facts.value.failure }}
          </p>
          <button
            v-if="
              selected.facts.value.action.kind === 'offered' &&
              selected.facts.value.state === 'awaitingConfirmation'
            "
            :disabled="s.busy || s.uncertain"
            @click="c.confirmPreparation(selected.facts.value.action.offer)"
          >
            本人确认此操作
          </button></template
        >
        <template v-else-if="selected.facts.kind === 'sampling'"
          ><p>
            {{ selected.facts.value.definition.title }} ·
            {{ selected.facts.value.definition.reference.id }}
          </p>
          <p>
            周期会话最近采集：{{
              selected.facts.value.latest.collectedAt == null
                ? "尚无保留事实"
                : new Date(
                    selected.facts.value.latest.collectedAt * 1000,
                  ).toLocaleString()
            }}
          </p>
          <p>
            质量 {{ selected.facts.value.latest.quality }} ·
            {{
              selected.facts.value.latest.enumerationComplete
                ? "枚举完整"
                : "结果部分或未完成"
            }}
          </p></template
        >
        <dl v-else>
          <dt>来源事实</dt>
          <dd>
            {{ selected.facts.value.source }} · {{ selected.facts.value.phase }}
          </dd>
          <dt>精确关联</dt>
          <dd>{{ JSON.stringify(selected.facts.value.links) }}</dd>
          <dt>设备及注册世代</dt>
          <dd>{{ JSON.stringify(selected.facts.value.target) }}</dd>
          <dt>执行 / 效果</dt>
          <dd>
            {{ selected.facts.value.execution ?? "未知" }} /
            {{ selected.facts.value.effect ?? "未核实" }}
          </dd>
          <dt>投递 / 上报</dt>
          <dd>
            {{ selected.facts.value.delivery ?? "未知" }} /
            {{ selected.facts.value.reporting ?? "未知" }}
          </dd>
          <dt>审计事实</dt>
          <dd>{{ selected.facts.value.auditOutcome }}（不证明设备效果）</dd>
        </dl>
        <section aria-label="父子步骤">
          <h3>步骤与材料</h3>
          <p v-if="!selected.nodes.length">该 owner 未提供可见子进度。</p>
          <ol>
            <li
              v-for="node in selected.nodes"
              :key="node.id"
              :data-node="node.id"
              :aria-current="s.node === node.id ? 'step' : undefined"
              tabindex="0"
            >
              <strong>{{ taskPhase[node.stage] ?? node.stage }}</strong>
              <p>
                {{ node.id
                }}<span v-if="node.parent"> · 父节点 {{ node.parent }}</span>
              </p>
              <p v-if="node.bytes != null">
                {{ node.bytes }} /
                {{ node.total == null ? "总量未知" : node.total }} 字节
              </p>
              <progress
                v-if="
                  node.bytes != null && node.total != null && node.total > 0
                "
                :value="node.bytes"
                :max="node.total"
              />
              <p v-if="node.exitCode != null">
                进程退出码：{{ node.exitCode }}（不代表效果核实）
              </p>
              <p v-if="node.detected">检测事实：{{ node.detected }}</p>
              <p v-if="node.quiescent != null">
                {{ node.quiescent ? "该调用的终止已证实" : "仍有活动未核实" }}
              </p>
              <p v-if="node.reason">
                {{ reasons[node.reason] ?? node.reason }}
              </p>
            </li>
          </ol>
        </section>
        <p v-if="selected.softwareState">
          独立软件检测事实：{{ selected.softwareState }}（执行与终止另行核实）
        </p>
        <p v-if="selected.collectionQuality != null">
          采集质量：{{ selected.collectionQuality }} ·
          {{ selected.collectionComplete ? "枚举完整" : "完整性未获证明" }}
        </p>
        <p v-if="selected.controlReason">
          {{ reasons[selected.controlReason] ?? selected.controlReason }}
        </p>
        <p>
          上传 ACK：{{
            selected.uploadAcknowledged == null
              ? "未知或不适用，独立于执行结果"
              : selected.uploadAcknowledged
                ? "已确认接收"
                : "尚未确认"
          }}
        </p>
        <p v-if="selected.missing.length">
          未覆盖关联：{{ selected.missing.join(" · ") }}
        </p>
        <button
          v-for="related in selected.related"
          :key="taskKey(related)"
          @click="c.openTask(related)"
        >
          查看关联 {{ taskKey(related) }}
        </button>
        <button @click="c.openTask(selected.reference, s.node)">
          查询原任务
        </button>
        <div class="task-actions">
          <button
            v-for="action in selected.allowedActions"
            :key="action"
            :disabled="s.busy || !!s.pendingControl"
            @click="c.control(selected, action)"
          >
            {{ labels[action] }}
          </button>
        </div>
      </section>
    </div>
  </section>
</template>
<style scoped>
.task-center {
  padding: 24px;
  max-width: 1280px;
  margin: auto;
}
header,
nav,
.task-actions {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}
header {
  justify-content: space-between;
}
nav {
  margin: 16px 0;
}
button[aria-current="page"] {
  font-weight: 700;
  border-bottom: 2px solid currentColor;
}
.task-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}
label {
  display: grid;
  gap: 4px;
  font-size: 12px;
}
input,
select {
  max-width: 190px;
  padding: 6px;
}
.task-center-layout {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) minmax(300px, 2fr);
  gap: 24px;
  margin-top: 20px;
}
.task-center-layout:not(.has-detail) {
  grid-template-columns: 1fr;
}
.task-center-list {
  max-height: 70vh;
  overflow: auto;
}
.task-center-list > button {
  display: grid;
  width: 100%;
  padding: 12px;
  gap: 6px;
  text-align: left;
  border-bottom: 1px solid #8883;
}
.task-center-detail {
  min-width: 0;
  overflow-wrap: anywhere;
}
dt {
  font-weight: 600;
  margin-top: 12px;
}
dd {
  margin: 4px 0;
}
li {
  margin-bottom: 12px;
}
[aria-current="step"] {
  outline: 2px solid currentColor;
}
@media (max-width: 760px) {
  .task-center {
    padding: 14px;
  }
  .task-center-layout {
    grid-template-columns: 1fr;
  }
  .has-detail .task-center-list {
    display: none;
  }
}
</style>
