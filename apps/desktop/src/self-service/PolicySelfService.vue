<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
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
const props = defineProps<{
  port: SelfServicePort | null;
  records: BackendRequest[];
}>();
const emit = defineEmits<{ changed: [] }>();
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
const validator = computed(() => {
  const schema = selected.value?.parameterSchema;
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
  Object.entries(selected.value?.parameterSources ?? {})
    .filter(([, source]) => source.kind === "input")
    .map(([name]) => ({
      name,
      schema:
        (selected.value?.parameterSchema as any)?.properties?.[name] ?? {},
    })),
);
const resolved = computed(() =>
  Object.fromEntries(
    Object.entries(selected.value?.parameterSources ?? {}).map(
      ([name, source]) => [
        name,
        source.kind === "fixed" ? source.value : choice.value?.parameters[name],
      ],
    ),
  ),
);
async function refresh() {
  if (!props.port || busy.value) return;
  busy.value = true;
  const current = epoch;
  try {
    const items: SelfServiceDirectoryItem[] = [];
    let after: string | null = null;
    for (let page = 0; page < 16; page++) {
      const value = await props.port.policy_directory({ after });
      if (current !== epoch) return;
      items.push(...value.items);
      after = value.nextCursor;
      if (!after) break;
    }
    directory.value = items;
    error.value = "";
    if (
      selected.value &&
      !items.some(
        (item) =>
          item.policyId === selected.value?.policyId &&
          item.policyRevision === selected.value?.policyRevision &&
          item.policyVersion === selected.value?.policyVersion,
      )
    ) {
      selected.value = undefined;
      choice.value = undefined;
    }
  } catch {
    if (current !== epoch) return;
    error.value = "请登录企业账户并连接执行服务后读取可用脚本。";
  } finally {
    if (current === epoch) busy.value = false;
  }
}
function select(item: SelfServiceDirectoryItem) {
  if (request.value) return;
  selected.value = item;
  choice.value = undefined;
  inputs.value = {};
  fieldErrors.value = new Map();
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
  if (!selected.value || busy.value || request.value) return;
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
      Object.entries(selected.value.parameterSources).map(([name, source]) => [
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
  if (!props.port || !choice.value || busy.value || request.value) return;
  request.value = choice.value.request;
  busy.value = true;
  const current = epoch;
  try {
    const value = await props.port.policy_invoke(choice.value);
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
  if (!props.port || busy.value) return;
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
    directory.value = [];
    selected.value = undefined;
    choice.value = undefined;
    request.value = "";
    status.value = undefined;
    void refresh();
  },
);
</script>
<template>
  <section class="policy-self-service" aria-label="企业脚本与工具">
    <div class="section-heading">
      <h3>企业脚本与工具</h3>
      <button :disabled="busy" @click="refresh">刷新脚本</button>
    </div>
    <p v-if="error" role="status">{{ error }}</p>
    <div class="catalog-grid">
      <article
        v-for="item in directory"
        :key="item.policyId"
        class="resource-card"
      >
        <button :disabled="busy || !!request" @click="select(item)">
          {{ item.selfService.displayName }}
        </button>
        <p>{{ item.selfService.description }}</p>
        <p>
          {{ item.selfService.category }} · 风险
          {{ item.selfService.riskLevel }}
        </p>
        <p>{{ item.selfService.allowAi ? "允许 AI 请求" : "仅供人工操作" }}</p>
      </article>
    </div>
    <form v-if="selected && !choice" @submit.prevent="preview">
      <h3>{{ selected.selfService.displayName }}</h3>
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
      ><button :disabled="busy">核对执行参数</button>
    </form>
    <section v-if="choice && !request" aria-label="人工执行确认">
      <h3>确认 {{ selected?.selfService.displayName }}</h3>
      <p>账号：{{ currentUser?.user.displayName }} · 当前登录设备</p>
      <p>前置条件：{{ selected?.selfService.prerequisites }}</p>
      <p>副作用：{{ selected?.selfService.sideEffects }}</p>
      <p>
        策略 {{ choice.policyId }} / revision {{ choice.policyRevision }} /
        执行版本 {{ choice.policyVersion }}
      </p>
      <p>
        资源 {{ selected?.resource.id }} / {{ selected?.resource.version }} /
        {{ selected?.resource.variant }}
      </p>
      <pre>{{ JSON.stringify(resolved, null, 2) }}</pre>
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
        :disabled="busy"
        @confirm="confirm"
        @cancel="cancel"
        @query="query"
      /><template v-else-if="status?.kind === 'run'"
        ><p>状态：{{ status.value.state }}</p>
        <template v-if="status.value.result"
          ><p>
            采集：{{
              new Date(status.value.result.collectedAt * 1000).toLocaleString()
            }}
            · {{ status.value.result.quality }}
          </p>
          <p>
            Schema：{{ status.value.result.schemaValid }} · 预算：{{
              status.value.result.budgetValid
            }}
            · 可信：{{ status.value.result.trusted }}
          </p>
          <pre>{{ JSON.stringify(status.value.result.output, null, 2) }}</pre>
          <p v-if="status.value.result.outputReference">
            结果仅有引用：{{ status.value.result.outputReference.sha256 }} /
            {{ status.value.result.outputReference.bytes }} 字节
          </p></template
        ></template
      >
    </section>
    <PolicyPreparation
      v-for="record in records.filter(
        (r) =>
          r.action.kind === 'policy' && r.action.choice.request !== request,
      )"
      :key="record.action.kind === 'policy' ? record.action.choice.request : ''"
      :record="record"
      :disabled="busy"
      @confirm="confirm"
      @cancel="cancel"
      @query="query"
    />
  </section>
</template>
