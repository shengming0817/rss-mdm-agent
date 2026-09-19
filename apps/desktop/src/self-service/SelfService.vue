<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import type { Controller } from "./controller";
import type { Decision, RequestView } from "./types";
import ParameterForm from "./ParameterForm.vue";
import PlanSummary from "./PlanSummary.vue";
import TaskDetail from "./TaskDetail.vue";
const props = defineProps<{ controller: Controller }>();
const c = props.controller;
const s = c.state;
const items = computed(
  () =>
    s.snapshot?.catalog.filter((item) =>
      s.page === "software"
        ? item.kind === "software"
        : item.kind !== "software",
    ) ?? [],
);
const task = computed(() =>
  s.snapshot?.requests.find((item) => item.plan.requestId === s.taskId),
);
function decision(value: Decision): string {
  switch (value) {
    case "allowed":
      return "允许";
    case "unknown":
      return "未知，需核实";
    case "missingCapability":
      return "缺少能力";
    case "unsupportedTarget":
      return "目标不适用";
    case "unresolvedResource":
      return "资源未解析";
    case "blocked":
      return "受限";
  }
}
function status(value: RequestView["status"]): string {
  switch (value) {
    case "waiting":
      return "等待交互";
    case "approval":
      return "等待管理员";
    case "complete":
      return "测试流程完成";
    case "stopped":
      return "流程停止";
    case "restartRequired":
      return "待重启提示";
    case "unknownEffect":
      return "效果未知";
  }
}
let polling: ReturnType<typeof setInterval>;
onMounted(() => {
  polling = setInterval(() => {
    if (!s.busy && !s.replying) void c.refresh();
  }, 1500);
  void c.refresh();
});
onUnmounted(() => clearInterval(polling));
</script>
<template>
  <div class="self-service">
    <div v-if="s.error" class="error" role="alert">{{ s.error }}</div>
    <div v-if="!s.snapshot" class="empty-state">
      <h1>正在连接桌面测试服务</h1>
      <p>服务不可用时不会切换为演示成功。</p>
      <button @click="c.refresh">重试连接</button>
    </div>
    <template v-else-if="s.page === 'home'">
      <section class="hero">
        <span class="eyebrow">YOUR EVERYDAY TOOLS</span>
        <h1>工作所需，<br />从这里开始。</h1>
        <p>
          查找软件、使用工具，并在一个地方跟进请求。<br />无需启用
          AI，也能完成自助测试流程。
        </p>
        <div class="actions">
          <button @click="c.navigate('software')">浏览软件</button
          ><button class="secondary" @click="c.navigate('tools')">
            打开工具中心
          </button>
        </div>
      </section>
      <div class="overview-grid">
        <article>
          <span class="card-index">01 / SOFTWARE</span>
          <h2>按需申请软件</h2>
          <p>先了解版本、适用条件和权限，再确认计划。</p>
        </article>
        <article>
          <span class="card-index">02 / TOOLS</span>
          <h2>清晰填写参数</h2>
          <p>目录提供同一套表单规则，敏感输入不会出现在摘要中。</p>
        </article>
        <article>
          <span class="card-index">03 / REQUESTS</span>
          <h2>随时查看进展</h2>
          <p>确认、隐私同意和管理员批准各自独立。</p>
        </article>
      </div>
      <p class="notice">
        这是固定测试服务。不会安装软件、运行系统命令或更改设备；退出应用后测试数据清空。
      </p>
    </template>
    <template v-else-if="s.page === 'software' || s.page === 'tools'">
      <div class="page-heading">
        <div>
          <span class="eyebrow">SELF SERVICE</span>
          <h1>{{ s.page === "software" ? "软件中心" : "工具中心" }}</h1>
          <p>选择项目，查看条件与精确计划。</p>
        </div>
        <button class="secondary" :disabled="!c.interactive" @click="c.refresh">
          刷新目录
        </button>
      </div>
      <div class="catalog-grid">
        <article v-for="item in items" :key="item.itemId" class="catalog-card">
          <span class="card-index"
            >{{ item.category }} / {{ item.resource.reference.revision }}</span
          >
          <h2>{{ item.name }}</h2>
          <p>{{ item.description }}</p>
          <dl class="decisions">
            <dt>可见</dt>
            <dd>{{ decision(item.display.visibility) }}</dd>
            <dt>可申请</dt>
            <dd>{{ decision(item.display.requestability) }}</dd>
            <dt>可执行</dt>
            <dd>{{ decision(item.display.executability) }}</dd>
          </dl>
          <p v-if="item.availability !== 'listed'" class="notice">
            {{
              item.availability === "expired"
                ? "目录已过期，只能浏览"
                : "已下架，只能浏览"
            }}
          </p>
          <button
            class="secondary"
            :disabled="s.busy || s.uncertain"
            @click="c.select(item)"
          >
            查看详情
          </button>
        </article>
      </div>
    </template>
    <template v-else-if="s.page === 'detail' && s.item">
      <button
        class="text-button"
        @click="c.navigate(s.item.kind === 'software' ? 'software' : 'tools')"
      >
        ← 返回目录
      </button>
      <h1>{{ s.item.name }}</h1>
      <p>{{ s.item.reason }}</p>
      <dl class="decisions">
        <dt>可见</dt>
        <dd>{{ decision(s.item.display.visibility) }}</dd>
        <dt>可申请</dt>
        <dd>{{ decision(s.item.display.requestability) }}</dd>
        <dt>可执行</dt>
        <dd>{{ decision(s.item.display.executability) }}</dd>
      </dl>
      <p v-if="!c.interactive" class="notice">
        浏览器仅展示页面，请在桌面应用中填写并提交测试请求。
      </p>
      <form novalidate @submit.prevent="c.prepare">
        <ParameterForm
          :fields="s.item.fields"
          :values="s.fields"
          :disabled="
            !c.interactive ||
            s.busy ||
            s.accepted ||
            s.uncertain ||
            s.item.display.requestability !== 'allowed'
          "
          prefix="draft"
          @change="c.change"
        />
        <div class="actions">
          <button
            :disabled="
              !c.interactive ||
              s.busy ||
              s.accepted ||
              s.uncertain ||
              s.item.display.requestability !== 'allowed'
            "
          >
            {{ s.busy ? "处理中…" : "预览确定性计划" }}</button
          ><button
            type="button"
            class="secondary"
            :disabled="!c.interactive || s.busy || s.uncertain"
            @click="c.select(s.item, true)"
          >
            新建请求
          </button>
        </div>
      </form>
      <template v-if="s.plan"
        ><PlanSummary :plan="s.plan" /><button
          :disabled="s.busy || s.accepted"
          @click="c.submit"
        >
          {{
            s.accepted
              ? "请求已接纳"
              : s.uncertain
                ? "按原请求重试提交"
                : s.item.kind === "software"
                  ? "提交测试申请"
                  : "提交测试请求"
          }}
        </button></template
      >
      <button v-if="s.uncertain" class="secondary" @click="c.refresh">
        查询原请求
      </button>
    </template>
    <template v-else-if="s.page === 'tasks'">
      <div v-if="s.uncertain" class="notice">
        提交结果尚不明确，可查询或重试原请求。
        <button :disabled="s.busy" @click="c.submit">按原请求重试提交</button>
      </div>
      <div class="page-heading">
        <div>
          <span class="eyebrow">REQUESTS</span>
          <h1>请求与任务</h1>
          <p>交互提示与任务结果分别记录。</p>
        </div>
        <button class="secondary" :disabled="!c.interactive" @click="c.refresh">
          刷新任务
        </button>
      </div>
      <p v-if="s.snapshot.requests.length === 0" class="empty-state">
        暂无请求。先从软件或工具目录开始。
      </p>
      <div class="task-layout">
        <div class="task-list">
          <button
            v-for="request in s.snapshot.requests"
            :key="request.plan.requestId"
            class="task-row secondary"
            :aria-pressed="s.taskId === request.plan.requestId"
            @click="s.taskId = request.plan.requestId"
          >
            <strong>{{ request.plan.title }}</strong
            ><span>{{ status(request.status) }}</span
            ><small class="identifier">{{ request.plan.requestId }}</small>
          </button>
        </div>
        <TaskDetail
          v-if="task"
          :key="task.plan.requestId"
          :task="task"
          :item="
            s.snapshot.catalog.find((item) => item.itemId === task?.plan.itemId)
          "
          :disabled="!c.interactive || s.busy || s.replying || s.replyUnknown"
          @approve="task && c.approve(task)"
          @cancel="task && c.cancel(task)"
          @respond="(id, answer) => task && c.respond(task, id, answer)"
        />
      </div>
      <button v-if="s.replyUnknown" @click="c.retryReply">重试原回答</button>
    </template>
    <template v-else-if="s.page === 'help'"
      ><h1>设备与帮助</h1>
      <p>{{ s.snapshot.targetLabel }}</p>
      <p>这是固定的模拟目标，不代表当前电脑的身份、权限或适用性。</p>
      <p>
        关闭窗口不会取消任务。S1 任务与会话分别保存在本地，重开后可查询原请求。
      </p>
      <p>
        测试批准仅授权精确的 S1 计划；当前没有真实安装、脚本执行或企业权限。
      </p></template
    >
  </div>
</template>
