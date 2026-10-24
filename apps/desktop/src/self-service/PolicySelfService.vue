<script setup lang="ts">
import PolicyExecutionContext from "./PolicyExecutionContext.vue";
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import Ajv, { type AnySchema } from "ajv";
import type {
  SelfServicePort,
  SelfServiceDirectoryItem,
  PolicyChoice,
  PolicyStatus,
  BackendRequest,
} from "./types";
import { currentUser } from "../test-users";
import PolicyPreparation from "./PolicyPreparation.vue";
const props = withDefaults(
  defineProps<{
    port: SelfServicePort | null;
    records: BackendRequest[];
    kind?: "enrollment" | "software" | "script";
    filter?: string;
    allowNewRequests?: boolean;
    allowConfirm?: boolean;
  }>(),
  { allowNewRequests: true, allowConfirm: true },
);
const emit = defineEmits<{ changed: [] }>();
watch(
  () => props.allowNewRequests,
  (allowed) => {
    if (allowed === false && !request.value) {
      selected.value = undefined;
      choice.value = undefined;
    }
  },
);
const category = ref("");
const categories = computed(() => [
  ...new Set(
    directory.value.flatMap((i) =>
      typeof i.selfService.presentation.category === "string"
        ? [i.selfService.presentation.category]
        : [],
    ),
  ),
]);
const visibleDirectory = computed(() =>
  directory.value.filter(
    (i) =>
      (!category.value ||
        i.selfService.presentation.category === category.value) &&
      `${i.selfService.presentation.displayName ?? ""} ${i.selfService.presentation.description ?? ""} ${i.policyId}`
        .toLocaleLowerCase()
        .includes((props.filter ?? "").trim().toLocaleLowerCase()),
  ),
);
const directory = ref<SelfServiceDirectoryItem[]>([]),
  selected = ref<SelfServiceDirectoryItem>(),
  inputs = ref<Record<string, string>>({}),
  choice = ref<PolicyChoice>(),
  request = ref(""),
  status = ref<PolicyStatus>(),
  busy = ref(false),
  error = ref("");
const fieldErrors = ref(new Map<string, string>());
const ajv = new Ajv({ strict: false, allErrors: true });
function isSchema(value: unknown): value is AnySchema {
  return (
    typeof value === "boolean" ||
    (typeof value === "object" && value !== null && !Array.isArray(value))
  );
}
const scriptTask = computed(() =>
  selected.value?.task.kind === "script" ? selected.value.task : undefined,
);
const parameterSources = computed(
  () => scriptTask.value?.parameterSources ?? {},
);
const parameterSchema = computed(
  () =>
    scriptTask.value?.parameterSchema ?? {
      type: "object",
      properties: {},
      additionalProperties: false,
    },
);
const validator = computed(() => {
  const schema = parameterSchema.value;
  return isSchema(schema) ? ajv.compile(schema) : undefined;
});
let epoch = 0;
const finished = computed(() => {
  if (status.value?.kind === "absent") return true;
  if (status.value?.kind === "preparation")
    return ["failed", "cancelled"].includes(status.value.value.state);
  if (status.value?.kind === "run")
    return (
      ["succeeded", "failed"].includes(status.value.value.state.execution) ||
      status.value.value.state.cancellation === "confirmed"
    );
  return false;
});
function editAbsent() {
  if (status.value?.kind !== "absent" || busy.value) return;
  request.value = "";
  status.value = undefined;
  choice.value = undefined;
  error.value = "";
  if (!selected.value) void refresh();
}
function newRequest() {
  if (!finished.value || busy.value) return;
  request.value = "";
  choice.value = undefined;
  selected.value = undefined;
  status.value = undefined;
  inputs.value = {};
  error.value = "";
  void refresh();
}
const fields = computed(() =>
  Object.entries(parameterSources.value)
    .filter(([, source]) => source.kind === "input")
    .map(([name]) => ({
      name,
      schema: (parameterSchema.value as any)?.properties?.[name] ?? {},
    })),
);
const resolved = computed(() =>
  Object.fromEntries(
    Object.entries(parameterSources.value).map(([name, source]) => [
      name,
      source.kind === "fixed" ? source.value : choice.value?.parameters[name],
    ]),
  ),
);
const nextCursor = ref<string | null>(null);
const directoryComplete = ref(false);
const directoryWindowed = ref(false);
const locators = new Map<string, string | null>();
const visited = new Set<string>();
async function refresh() {
  await loadDirectory(false);
}
async function loadMore() {
  await loadDirectory(true);
}
async function loadDirectory(append: boolean) {
  if (!props.port || busy.value || (append && !nextCursor.value)) return;
  if (!append) {
    ++epoch;
    directory.value = [];
    nextCursor.value = null;
    directoryComplete.value = false;
    directoryWindowed.value = false;
    locators.clear();
    visited.clear();
    if (!request.value) {
      selected.value = undefined;
      choice.value = undefined;
    }
  }
  busy.value = true;
  const current = epoch;
  const after = append ? nextCursor.value : null;
  try {
    if (visited.size >= 2048) throw new Error("capacity");
    const value = await props.port.policy_directory({ after });
    if (current !== epoch) return;
    if (
      value.nextCursor &&
      (value.nextCursor === after || visited.has(value.nextCursor))
    )
      throw new Error("cursor");
    if (after) visited.add(after);
    const items = value.items.filter((item) =>
      props.kind
        ? item.task.kind === props.kind
        : item.task.kind !== "enrollment",
    );
    for (const item of items) locators.set(item.policyId, after);
    const merged = new Map(
      directory.value.map((item) => [item.policyId, item]),
    );
    for (const item of items) merged.set(item.policyId, item);
    if (merged.size > 1024) directoryWindowed.value = true;
    directory.value = [...merged.values()].slice(-1024);
    const retained = new Set(directory.value.map((item) => item.policyId));
    for (const id of locators.keys())
      if (!retained.has(id) && id !== selected.value?.policyId)
        locators.delete(id);
    nextCursor.value = value.nextCursor;
    directoryComplete.value = !value.nextCursor && !directoryWindowed.value;
    error.value = "";
  } catch {
    if (current === epoch)
      error.value =
        "目录读取未完成；已加载条目保留，可重试本页。刷新可重新开始发现。";
  } finally {
    if (current === epoch) busy.value = false;
  }
}
onBeforeUnmount(() => {
  ++epoch;
});
function select(item: SelfServiceDirectoryItem) {
  if (request.value || props.allowNewRequests === false) return;
  selected.value = item;
  choice.value = undefined;
  inputs.value = {};
  fieldErrors.value = new Map();
}
async function runScript() {
  preview();
  if (choice.value) await invoke();
}
function options(schema: any): unknown[] | undefined {
  return (
    schema.enum ??
    ("const" in schema
      ? [schema.const]
      : schema.type === "boolean"
        ? [true, false]
        : undefined)
  );
}
function preview() {
  if (
    !selected.value ||
    busy.value ||
    request.value ||
    props.allowNewRequests === false
  )
    return;
  try {
    fieldErrors.value = new Map();
    const parameters = new Map<string, unknown>();
    for (const field of fields.value) {
      try {
        const value =
          options(field.schema) || field.schema.type !== "string"
            ? JSON.parse(inputs.value[field.name] ?? "", (_key, value) => {
                if (typeof value === "number" && !Number.isSafeInteger(value))
                  throw new Error("不支持的整数值");
                return value;
              })
            : (inputs.value[field.name] ?? "");
        parameters.set(field.name, value);
      } catch {
        fieldErrors.value.set(field.name, "请填写有效的字段值。");
      }
    }
    const resolvedParameters = Object.fromEntries(
      Object.entries(parameterSources.value).map(([name, source]) => [
        name,
        source.kind === "fixed" ? source.value : parameters.get(name),
      ]),
    );
    if (!validator.value?.(resolvedParameters)) {
      for (const issue of validator.value?.errors ?? []) {
        const name =
          issue.instancePath.split("/").at(1) || issue.params.missingProperty;
        if (name)
          fieldErrors.value.set(name, `不符合字段要求：${issue.message}`);
      }
    }
    if (fieldErrors.value.size) {
      error.value = "请修正标出的参数后再核对。";
      return;
    }
    choice.value = {
      request: crypto.randomUUID(),
      policyId: selected.value.policyId,
      policyRevision: selected.value.policyRevision,
      policyVersion: selected.value.policyVersion,
      parameters: Object.fromEntries(parameters),
    };
    error.value = "";
  } catch {
    error.value = "参数格式无效，请按字段要求填写。";
  }
}
async function query(id?: string) {
  if (id) {
    request.value = id;
    status.value = undefined;
    choice.value = undefined;
    selected.value = undefined;
  }
  if (!props.port || !request.value) return;
  const current = epoch;
  try {
    const original = request.value;
    const value = await props.port.policy_status({ requestId: original });
    if (current !== epoch || request.value !== original) return;
    status.value = value;
    emit("changed");
  } catch {
    if (current !== epoch) return;
    error.value = "原请求状态未知；请稍后查询，勿重复执行。";
  }
}
async function invoke() {
  if (
    !props.port ||
    !choice.value ||
    busy.value ||
    request.value ||
    props.allowNewRequests === false
  )
    return;
  request.value = choice.value.request;
  busy.value = true;
  const current = epoch;
  try {
    const value = await props.port.policy_invoke({
      choice: choice.value,
      locator: { after: locators.get(choice.value.policyId) ?? null },
    });
    if (current !== epoch) return;
    status.value = { kind: "preparation", value };
    emit("changed");
  } catch {
    if (current !== epoch) return;
    error.value = "提交回执未确认；保留原请求并查询状态，勿重复执行。";
  } finally {
    if (current === epoch) busy.value = false;
  }
}
async function confirm(id: string) {
  if (!props.port || busy.value || props.allowConfirm === false) return;
  busy.value = true;
  request.value = id;
  const current = epoch;
  try {
    const value = await props.port.policy_confirm({ requestId: id });
    if (current !== epoch) return;
    status.value = { kind: "preparation", value };
    emit("changed");
  } catch {
    if (current !== epoch) return;
    error.value = "确认回执未知；请查询原请求，勿重复确认。";
  } finally {
    if (current === epoch) busy.value = false;
  }
}
async function cancel(id: string) {
  if (!props.port || busy.value) return;
  busy.value = true;
  const current = epoch;
  try {
    await props.port.cancel({ requestId: id });
    await query(id);
    if (current !== epoch) return;
    emit("changed");
  } catch {
    if (current !== epoch) return;
    error.value = "撤销回执未确认，请查询原请求。";
  } finally {
    if (current === epoch) busy.value = false;
  }
}
onMounted(refresh);
watch(
  () => currentUser.value?.generation,
  () => {
    epoch += 1;
    busy.value = false;
    inputs.value = {};
    fieldErrors.value = new Map();
    error.value = "";
    directory.value = [];
    selected.value = undefined;
    choice.value = undefined;
    request.value = "";
    status.value = undefined;
    void refresh();
  },
);
function presentationText(value: unknown, fallback = "未提供"): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}
</script>
<template>
  <section class="policy-self-service" aria-label="企业自助任务">
    <div class="section-heading">
      <h3>{{ kind === "enrollment" ? "MDM 注册" : "企业自助任务" }}</h3>
      <button :disabled="busy" @click="refresh">刷新任务</button>
    </div>
    <label v-if="kind === 'software'"
      >发布分类
      <select v-model="category">
        <option value="">全部发布分类</option>
        <option v-for="name in categories" :key="name">{{ name }}</option>
      </select></label
    >
    <p v-if="kind === 'software'">
      企业发布目标与本机安装记录独立显示。当前发布接口未提供可核实的安装关联时，关联状态为未知。
    </p>
    <p v-if="directoryWindowed">
      仅保留最近加载的 1024 项；搜索与计数不代表完整目录，可刷新重新浏览。
    </p>
    <p role="status">
      当前加载 {{ directory.length }} 条；{{
        directoryComplete
          ? "目录已读取完毕"
          : "目录尚未完整，搜索与计数仅覆盖当前加载范围"
      }}。
    </p>
    <button v-if="nextCursor" :disabled="busy" @click="loadMore">
      继续加载
    </button>
    <button v-if="error && !nextCursor" :disabled="busy" @click="refresh">
      重试读取
    </button>
    <p v-if="error" role="status">{{ error }}</p>
    <p
      v-if="
        kind === 'enrollment' &&
        !busy &&
        !error &&
        !directory.length &&
        !records.some(
          (r) =>
            r.action.kind === 'policy' &&
            r.action.directory.task.kind === 'enrollment',
        )
      "
    >
      当前没有组织授权的 MDM 注册入口。
    </p>
    <div class="catalog-grid" v-if="allowNewRequests !== false">
      <article
        v-for="item in visibleDirectory"
        :key="item.policyId"
        class="resource-card"
      >
        <button :disabled="busy || !!request" @click="select(item)">
          {{
            kind === "enrollment"
              ? "注册 MDM"
              : presentationText(
                  item.selfService.presentation.displayName,
                  `策略 ${item.policyId}`,
                )
          }}
        </button>
        <p>{{ presentationText(item.selfService.presentation.description) }}</p>
        <p>
          {{ presentationText(item.selfService.presentation.category) }} · 风险
          {{ item.selfService.riskLevel }}
        </p>
        <p>
          {{
            item.subject.kind === "device"
              ? "设备准入 · 无需企业登录"
              : "企业用户准入 · 需要有效企业会话"
          }}
        </p>
        <p>{{ item.selfService.allowAi ? "允许 AI 请求" : "仅供人工操作" }}</p>
      </article>
    </div>
    <form
      v-if="selected && !choice && allowNewRequests !== false"
      @submit.prevent="scriptTask ? runScript() : preview()"
    >
      <h3>
        {{
          presentationText(
            selected.selfService.presentation.displayName,
            `策略 ${selected.policyId}`,
          )
        }}
      </h3>
      <label v-for="field in fields" :key="field.name"
        >{{ field.schema.title ?? field.name
        }}<span>{{ field.schema.description }}</span
        ><select
          v-if="options(field.schema)"
          v-model="inputs[field.name]"
          required
        >
          <option disabled value="">请选择</option>
          <option
            v-for="value in options(field.schema)"
            :key="JSON.stringify(value)"
            :value="JSON.stringify(value)"
          >
            {{ typeof value === "string" ? value : JSON.stringify(value) }}
          </option></select
        ><input
          v-else-if="field.schema.type === 'string'"
          v-model="inputs[field.name]"
          required
          :minlength="field.schema.minLength"
          :maxlength="field.schema.maxLength"
        /><textarea
          v-else
          v-model="inputs[field.name]"
          placeholder="按字段要求填写 JSON 值"
        /><small
          v-if="
            field.schema.minLength != null || field.schema.maxLength != null
          "
        >
          长度：{{ field.schema.minLength ?? 0 }} 至
          {{ field.schema.maxLength ?? "不限" }} </small
        ><small
          v-if="field.schema.minimum != null || field.schema.maximum != null"
        >
          范围：{{ field.schema.minimum ?? "不限" }} 至
          {{ field.schema.maximum ?? "不限" }} </small
        ><span v-if="fieldErrors.get(field.name)" role="alert">{{
          fieldErrors.get(field.name)
        }}</span></label
      ><template v-if="scriptTask">
        <p>{{ selected.selfService.presentation.prerequisites }}</p>
        <p>{{ selected.selfService.presentation.sideEffects }}</p>
        <PolicyExecutionContext :task="selected.task" />
        <p v-for="(source, name) in parameterSources" :key="name">
          <template v-if="source.kind === 'fixed'">{{ name }}（固定）：{{ JSON.stringify(source.value) }}</template>
        </p>
      </template><button :disabled="busy">{{ scriptTask ? '运行' : '核对执行参数' }}</button>
    </form>
    <section
      v-if="choice && !request && !scriptTask && allowNewRequests !== false"
      aria-label="人工执行确认"
    >
      <h3>
        确认
        {{
          presentationText(
            selected?.selfService.presentation.displayName,
            `策略 ${selected?.policyId}`,
          )
        }}
      </h3>
      <p>
        准入主体：{{
          selected?.subject.kind === "device"
            ? "设备准入 · 无需企业登录"
            : "企业用户准入"
        }}
      </p>
      <p v-if="selected?.subject.kind === 'user'">
        企业身份：{{ selected.subject.user.tenantId }} /
        {{ selected.subject.user.principalId }}
      </p>
      <p>
        前置条件：{{
          presentationText(selected?.selfService.presentation.prerequisites)
        }}
      </p>
      <p>
        副作用：{{
          presentationText(selected?.selfService.presentation.sideEffects)
        }}
      </p>
      <p>
        策略 {{ choice.policyId }} / revision {{ choice.policyRevision }} /
        执行版本 {{ choice.policyVersion }}
      </p>
      <p v-if="selected && selected.task.kind !== 'enrollment'">
        资源 {{ selected.task.resource.id }} /
        {{ selected.task.resource.version }} /
        {{ selected.task.resource.variant }}
      </p>
      <PolicyExecutionContext v-if="selected" :task="selected.task" />
      <pre v-if="selected?.task.kind === 'script'">{{
        JSON.stringify(resolved, null, 2)
      }}</pre>
      <button :disabled="busy" @click="invoke">确认并执行</button
      ><button :disabled="busy" @click="choice = undefined">修改参数</button>
    </section>
    <section v-if="request">
      <p>原请求：{{ request }}</p>
      <template v-if="status?.kind === 'absent'">
        <p>原请求未创建；可以修改参数或重新选择脚本。</p>
        <button :disabled="busy" @click="editAbsent">修改参数</button>
      </template>
      <button v-if="finished" :disabled="busy" @click="newRequest">
        发起新任务
      </button>
      <button :disabled="busy" @click="query()">查询原请求</button
      ><PolicyPreparation
        v-if="status?.kind === 'preparation'"
        :record="status.value"
        :confirm-disabled="allowConfirm === false"
        :disabled="busy"
        @confirm="confirm"
        @cancel="cancel"
        @query="query"
      /><template v-else-if="status?.kind === 'run'"
        ><p>状态：{{ status.value.state }}</p>
        <template v-if="status.value.result?.kind === 'script'"
          ><p>
            采集：{{
              new Date(
                status.value.result.evidence.collectedAt * 1000,
              ).toLocaleString()
            }}
            · {{ status.value.result.evidence.quality }}
          </p>
          <p>
            Schema：{{ status.value.result.evidence.schemaValid }} · 预算：{{
              status.value.result.evidence.budgetValid
            }}
            · 可信：{{ status.value.result.evidence.trusted }}
          </p>
          <pre>{{
            JSON.stringify(status.value.result.evidence.output, null, 2)
          }}</pre>
          <p v-if="status.value.result.evidence.outputReference">
            结果仅有引用：{{
              status.value.result.evidence.outputReference.sha256
            }}
            / {{ status.value.result.evidence.outputReference.bytes }} 字节
          </p></template
        >
        <template v-else-if="status.value.result?.kind === 'enrollment'">
          <p>
            {{
              status.value.result.outcome === "opened"
                ? "已打开系统注册流程，请完成系统确认。最终状态以注册检测为准。"
                : `注册入口状态：${status.value.result.outcome}`
            }}
          </p>
        </template>
        <template v-else-if="status.value.result?.kind === 'software'">
          <p>软件效果：{{ status.value.result.effect }}</p>
        </template></template
      >
    </section>
    <PolicyPreparation
      v-for="record in records.filter(
        (r) =>
          r.action.kind === 'policy' &&
          r.action.choice.request !== request &&
          (kind
            ? r.action.directory.task.kind === kind
            : r.action.directory.task.kind !== 'enrollment'),
      )"
      :key="record.action.kind === 'policy' ? record.action.choice.request : ''"
      :record="record"
      :confirm-disabled="allowConfirm === false"
      :disabled="busy"
      @confirm="confirm"
      @cancel="cancel"
      @query="query"
    />
  </section>
</template>
