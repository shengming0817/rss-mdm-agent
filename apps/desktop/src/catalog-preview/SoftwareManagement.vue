<script setup lang="ts">
import { reactive, computed, inject, nextTick, ref } from "vue";
import { Activity, UiDialog, UiSearchInput } from "@rss-mdm-agent/ui";
import {
  softwarePreviewKey,
  type SoftwarePreviewAction,
  type SoftwarePreview,
} from "./software-preview";
import "./software.css";
import AppArtwork from "./AppArtwork.vue";
import type { PageNavigation } from "../workspace/page-navigation";
const emit = defineEmits<{
  navigation: [state: PageNavigation];
  navigate: [page: string];
}>();
const preview = inject<SoftwarePreview | undefined>(
  softwarePreviewKey,
  undefined,
);
const query = ref("");
const tab = ref("discover");
const librarySections = [
  { id: "discover", title: "发现" },
  { id: "all", title: "全部应用" },
  { id: "installed", title: "已安装" },
  { id: "updates", title: "更新" },
];
const category = ref("");
const selected = ref("");
const heading = ref<HTMLElement>();
const listing = ref<HTMLElement>();
const search = ref<InstanceType<typeof UiSearchInput>>();
const notice = ref("");
const help = ref("");
const pending = ref<{
  id: string;
  action: SoftwarePreviewAction;
  revision: number;
}>();
const items = computed(() => preview?.items ?? []);
const updates = computed(
  () =>
    items.value.filter((item) => item.installed && item.version !== item.latest)
      .length,
);
const current = computed(() =>
  items.value.find((item) => item.id === selected.value),
);
const target = computed(() =>
  items.value.find((item) => item.id === pending.value?.id),
);
const categories = [
  { title: "效率办公", kind: "office", ids: ["workspace", "notes", "sync"] },
  { title: "创意设计", kind: "design", ids: ["design"] },
  { title: "网络工具", kind: "network", ids: ["browser"] },
  { title: "开发工具", kind: "code", ids: [] },
  { title: "系统工具", kind: "tools", ids: ["pdf", "sync"] },
];
const sections = [
  ...librarySections.map((item) => ({ ...item, group: "软件库" })),
  ...categories.map((item) => ({
    id: `category:${item.title}`,
    title: item.title,
    group: "按类别浏览",
    kind: item.kind,
  })),
];
const filtered = computed(() => {
  const result = items.value.filter(
    (item) =>
      (tab.value === "updates"
        ? item.installed && item.version !== item.latest
        : tab.value === "installed"
          ? item.installed
          : true) &&
      (!category.value ||
        categories
          .find((e) => e.title === category.value)
          ?.ids.includes(item.id)) &&
      item.name.toLowerCase().includes(query.value.trim().toLowerCase()),
  );
  return tab.value === "discover" && !query.value
    ? ["workspace", "notes", "design", "browser"].flatMap((id) =>
        result.filter((item) => item.id === id),
      )
    : result;
});
function browseCategory(name: string) {
  openSection("all");
  category.value = name;
}
const section = computed(() =>
  category.value ? `category:${category.value}` : tab.value,
);
const path = computed(() => [
  ...(tab.value !== "discover"
    ? [
        {
          id: tab.value,
          title: sections.find((e) => e.id === tab.value)?.title ?? "应用",
        },
      ]
    : []),
  ...(category.value
    ? [{ id: `category:${category.value}`, title: category.value }]
    : []),
  ...(current.value
    ? [{ id: current.value.id, title: current.value.name }]
    : []),
]);
function reset() {
  tab.value = "discover";
  selected.value = "";
  query.value = "";
  category.value = "";
}
function openSection(id: string) {
  if (items.value.some((e) => e.id === id)) {
    void inspect(id);
    return;
  }
  const targetCategory = categories.find(
    (item) => `category:${item.title}` === id,
  );
  if (targetCategory) {
    tab.value = "all";
    category.value = targetCategory.title;
    selected.value = "";
    query.value = "";
    return;
  }
  if (librarySections.some((e) => e.id === id)) {
    tab.value = id;
    selected.value = "";
    query.value = "";
    category.value = "";
  }
}

const actionName = computed(() =>
  pending.value?.action === "install"
    ? "安装"
    : pending.value?.action === "update"
      ? "更新"
      : "卸载",
);
async function inspect(id: string) {
  selected.value = id;
  notice.value = "";
  await nextTick();
  heading.value?.focus();
}
async function back() {
  const id = selected.value;
  selected.value = "";
  await nextTick();
  const button = Array.from(
    listing.value?.querySelectorAll<HTMLButtonElement>(
      "button[data-software]",
    ) ?? [],
  ).find((button) => button.dataset.software === id);
  if (button) button.focus();
  else search.value?.focus();
}
function request(id: string, action: SoftwarePreviewAction) {
  if (preview) pending.value = { id, action, revision: preview.revision };
}
function confirm() {
  const request = pending.value;
  if (!request || !preview) return;
  const name = target.value?.name ?? "软件";
  const label = actionName.value;
  const applied = preview.apply(request.id, request.action, request.revision);
  pending.value = undefined;
  notice.value = applied
    ? `预览：${name}已${label}，未操作真实软件。`
    : "样例状态已变化，请重新查看后再试。";
}
function launch(name: string) {
  notice.value = `预览：打开 ${name}。未启动真实应用。`;
}
emit(
  "navigation",
  reactive({
    reset,
    openSection,
    sections,
    section,
    path,
    resources: computed(() =>
      (current.value ? [current.value] : filtered.value).map((item) => ({
        id: item.id,
        title: item.name,
        description: item.description,
        version: item.version ?? item.latest,
        status: item.installed ? "样例已安装" : "样例未安装",
        source: "设计预览",
      })),
    ),
    searchEntries: computed(() =>
      items.value.map((e) => ({ id: e.id, title: e.name })),
    ),
    setQuery: (value: string) => {
      query.value = value;
      selected.value = "";
    },
  }),
);
</script>
<template>
  <section class="software-reference design-page" aria-label="软件管理预览">
    <div class="software-workspace">
      <template v-if="!current">
        <div v-if="tab === 'discover' && !query" class="store-editorial">
          <div class="design-hero store-hero">
            <div>
              <span class="design-eyebrow">编辑精选 · 高效工作</span>
              <h2>好工具，<br />让工作轻一点。</h2>
              <p>沟通、记录、专注。<br />为你的每一天，找到恰到好处的应用。</p>
              <button
                type="button"
                class="design-button"
                @click="openSection('all')"
              >
                探索效率应用 →
              </button>
            </div>
            <div class="floating-apps" aria-hidden="true">
              <span>N</span><span>W</span><span>◎</span>
            </div>
          </div>
          <div class="creative-story">
            <span class="design-eyebrow">CREATIVE SPACE</span>
            <h2>每个想法，<br />都值得被看见。</h2>
            <p>从第一笔灵感，到下一件作品。<br />发现你的创作搭档。</p>
            <button
              type="button"
              @click="preview ? inspect('design') : openSection('all')"
            >
              认识 Design Studio →
            </button>
            <div class="creative-flower" aria-hidden="true">
              <i /><i /><i /><i />
            </div>
          </div>
        </div>
        <div class="section-heading">
          <div>
            <h3>
              {{
                tab === "discover"
                  ? "开始你的高效工作日"
                  : category || sections.find((e) => e.id === tab)?.title
              }}
            </h3>
            <p>
              {{
                tab === "discover"
                  ? "好用的应用，让日常变得不一样。"
                  : "你的应用，一目了然。"
              }}
            </p>
          </div>
          <UiSearchInput
            ref="search"
            v-model="query"
            aria-label="搜索软件管理目录"
            placeholder="搜索应用"
          />
        </div>
        <div v-if="filtered.length" ref="listing" class="software-list">
          <article
            v-for="item in filtered"
            :key="item.id"
            class="software-item"
          >
            <button
              type="button"
              class="artwork-link"
              :aria-label="`查看 ${item.name} 详情`"
              @click="inspect(item.id)"
            >
              <AppArtwork :kind="item.id" />
            </button>
            <div class="store-card-footer">
              <span class="software-logo" :class="item.id">{{
                item.letter
              }}</span>
              <div class="software-item-copy">
                <button
                  type="button"
                  class="software-name"
                  :data-software="item.id"
                  @click="inspect(item.id)"
                >
                  {{ item.name }}
                </button>
                <p>{{ item.description }}</p>
              </div>
              <button
                type="button"
                class="store-action"
                @click="
                  !item.installed
                    ? request(item.id, 'install')
                    : tab === 'updates'
                      ? request(item.id, 'update')
                      : launch(item.name)
                "
              >
                {{
                  !item.installed ? "获取" : tab === "updates" ? "更新" : "打开"
                }}
              </button>
            </div>
          </article>
        </div>
        <p v-else class="software-empty">
          {{
            !preview
              ? "软件清单尚未接入；开发预览提供设计样例。"
              : query
                ? "没有匹配的软件"
                : tab === "updates"
                  ? "软件均为最新版本（样例）"
                  : "这里还没有软件"
          }}
        </p>
        <template v-if="tab === 'discover' && !query"
          ><div class="section-heading"><h3>按你的需要，发现应用</h3></div>
          <div class="store-categories">
            <button
              v-for="item in [
                '效率办公',
                '创意设计',
                '网络工具',
                '开发工具',
                '系统工具',
              ]"
              :key="item"
              @click="browseCategory(item)"
            >
              {{ item }} <span>›</span>
            </button>
          </div>
          <button class="library-banner" @click="openSection('installed')">
            <span
              ><strong>你的工作搭档，已经准备好了。</strong
              ><small>查看已安装应用，保持常用工具及时更新。</small></span
            ><span>打开我的软件库 →</span>
          </button></template
        >
      </template>
      <template v-else>
        <button
          class="text-link software-detail-back"
          type="button"
          @click="back"
        >
          ‹ 返回应用目录
        </button>
        <section class="software-detail-hero">
          <span class="software-logo" :class="current.color">{{
            current.letter
          }}</span>
          <div class="software-item-copy">
            <h2 ref="heading" tabindex="-1">{{ current.name }}</h2>
            <p>
              {{
                current.installed
                  ? `已安装 · 版本 ${current.version}`
                  : "尚未安装"
              }}
              · {{ current.size }}
            </p>
          </div>
          <div class="software-item-actions">
            <template v-if="current.installed"
              ><button
                class="software-button primary"
                type="button"
                @click="launch(current.name)"
              >
                打开</button
              ><button
                v-if="current.version !== current.latest"
                class="software-button"
                type="button"
                @click="request(current.id, 'update')"
              >
                更新至 {{ current.latest }}</button
              ><button
                class="software-button danger"
                type="button"
                :disabled="current.managed"
                :title="
                  current.managed ? '由组织管理，无法在此卸载' : undefined
                "
                @click="request(current.id, 'uninstall')"
              >
                卸载
              </button></template
            ><button
              v-else
              class="software-button primary"
              type="button"
              @click="request(current.id, 'install')"
            >
              安装
            </button>
          </div>
        </section>
        <div v-if="current.id === 'workspace'" class="software-problem">
          <span class="software-problem-icon"
            ><Activity :size="19" aria-hidden="true"
          /></span>
          <div>
            <strong>Workspace 无法连接</strong>
            <p>Wi-Fi 仍然在线，可以检查应用的网络连接。<span>（样例）</span></p>
          </div>
          <button
            class="software-button soft"
            type="button"
            @click="help = current.name"
          >
            检查问题
          </button>
        </div>
        <div class="software-info-grid">
          <section class="software-info">
            <h3>关于此软件</h3>
            <p>{{ current.description }}</p>
            <dl>
              <div>
                <dt>来源</dt>
                <dd>工作软件目录</dd>
              </div>
              <div>
                <dt>最新版本</dt>
                <dd>{{ current.latest }}</dd>
              </div>
              <div>
                <dt>管理方式</dt>
                <dd>
                  {{ current.managed ? "由组织管理" : "可自行安装和移除" }}
                </dd>
              </div>
            </dl>
          </section>
          <section class="software-info">
            <h3>需要帮助？</h3>
            <p>无法打开、连接失败或运行异常时，可以检查问题。</p>
            <button
              class="software-button soft"
              type="button"
              @click="help = current.name"
            >
              检查问题</button
            ><small>安装、更新和卸载均为设计预览；未操作真实软件。</small>
          </section>
        </div>
      </template>
      <p v-if="notice" role="status" class="software-notice">{{ notice }}</p>
    </div>

    <UiDialog
      :open="!!pending && !!target"
      :title="`${actionName} ${target?.name ?? ''}`"
      @update:open="
        (value) => {
          if (!value) pending = undefined;
        }
      "
      ><template v-if="target"
        ><p v-if="pending?.action === 'install'">
          将安装版本 {{ target.latest }}，大小 {{ target.size }}。
        </p>
        <p v-else-if="pending?.action === 'update'">
          将从 {{ target.version }} 更新到 {{ target.latest }}。
        </p>
        <p v-else>将从这台电脑移除此软件。实际产品会在执行前说明数据影响。</p>
        <p>此预览只模拟操作，不修改你的电脑。</p>
        <div class="software-dialog-actions">
          <button
            type="button"
            class="software-button"
            @click="pending = undefined"
          >
            取消</button
          ><button
            type="button"
            class="software-button primary"
            @click="confirm"
          >
            确认{{ actionName }}
          </button>
        </div></template
      ></UiDialog
    >
    <UiDialog
      :open="!!help"
      :title="`${help} 帮助`"
      @update:open="
        (value) => {
          if (!value) help = '';
        }
      "
      ><p>请描述无法打开、连接失败或运行异常的问题。</p>
      <p>当前软件设计预览没有执行真实检查，不会自动修改软件或系统设置。</p>
      <div class="software-dialog-actions">
        <button type="button" class="software-button" @click="help = ''">
          知道了</button
        ><button
          type="button"
          class="software-button primary"
          @click="
            help = '';
            emit('navigate', 'assistant');
          "
        >
          在电脑助手中继续
        </button>
      </div></UiDialog
    >
  </section>
</template>
