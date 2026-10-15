<script setup lang="ts">
import { computed, ref } from "vue";
import {
  UiProvider,
  UiText,
  UiIcon,
  UiStack,
  UiGrid,
  UiSurface,
  UiDivider,
  UiButton,
  UiLink,
  UiField,
  UiTextInput,
  UiNumberInput,
  UiSearchInput,
  UiCheckbox,
  UiRadioGroup,
  UiSwitch,
  UiSelect,
  UiCombobox,
  UiSlider,
  UiDateInput,
  UiTimeInput,
  UiList,
  UiTree,
  UiTable,
  UiTabs,
  UiNavigation,
  UiCollapsible,
  UiSplitPane,
  UiToolbar,
  UiToolbarButton,
  UiMenu,
  UiPopover,
  UiTooltip,
  UiDialog,
  UiProgress,
  UiSpinner,
  UiSteps,
  UiBadge,
  UiMessage,
  UiToasts,
  UiEmptyState,
  UiSkeleton,
  UiChart,
  Sparkles,
  type UiPlatform,
  type UiTheme,
  type UiTreeNode,
} from "../../packages/ui/src";
import { foundationCatalog } from "./foundation-catalog";
const props = defineProps<{ platform: UiPlatform; initialFamily?: string }>();
const family = ref(props.initialFamily ?? "U01"),
  theme = ref<UiTheme>("system");
const disabled = ref(false),
  readonly = ref(false),
  invalid = ref(false),
  busy = ref(false),
  reducedMotion = ref(false),
  highContrast = ref(false);
const text = ref("示例文字"),
  number = ref<number | null>(25),
  query = ref(""),
  selected = ref("a"),
  checked = ref<boolean | "indeterminate">(false),
  on = ref(false),
  date = ref("2026-10-15"),
  time = ref("09:30"),
  selection = ref<string[]>([]),
  expanded = ref<string[]>([]),
  tab = ref("a"),
  open = ref(false),
  menu = ref(false),
  popover = ref(false),
  ratio = ref(0.4),
  visibleSeries = ref(["a", "b"]);
const empty = ref(false),
  events = ref("尚无操作"),
  toasts = ref<{ id: string; text: string }[]>([]),
  sort = ref<{ key: string; direction: "ascending" | "descending" }>(),
  page = ref(1);
const options = [
  { value: "a", label: "选项 A" },
  { value: "b", label: "选项 B" },
  { value: "c", label: "禁用选项", disabled: true },
];
const items = options.map((o) => ({
  id: o.value,
  label: o.label,
  disabled: o.disabled,
}));
const nodes = ref<UiTreeNode[]>([
  { id: "parent", label: "分组", hasChildren: true },
  { id: "leaf", label: "独立项目" },
]);
const rows = computed(() =>
  empty.value
    ? []
    : [...items].sort((a, b) =>
        sort.value?.direction === "descending"
          ? b.label.localeCompare(a.label)
          : a.label.localeCompare(b.label),
      ),
);
const current = computed(
  () => foundationCatalog.find((item) => item.id === family.value)!,
);
const log = (value: string) => {
  events.value = value;
};
function load(id: string) {
  const node = nodes.value.find((n) => n.id === id);
  if (node) node.children = [{ id: "child", label: "由调用方加载的子项" }];
  log(`加载 ${id}`);
}
</script>
<template>
  <UiProvider
    :platform="platform"
    :theme="theme"
    :reduced-motion="reducedMotion"
    :high-contrast="highContrast"
  >
    <section class="foundation-gallery" :data-platform="platform">
      <header>
        <h1>
          {{ platform === "windows" ? "Windows 11" : "macOS 26" }} · 基础控件
        </h1>
        <p>
          真实 Vue 组件 · 功能无关 ·
          {{
            platform === "windows"
              ? "当前宿主上的外观与交互预览，非 Windows 原生验收"
              : "Web 控件平台适配，不是系统授权界面"
          }}
        </p>
      </header>
      <div class="foundation-layout">
        <UiNavigation
          :items="foundationCatalog"
          v-model="family"
          label="组件族"
        />
        <main :data-family="family">
          <h2>{{ current.id }} · {{ current.label }}</h2>
          <p>{{ current.states }}</p>
          <div class="foundation-switches">
            <label><input v-model="disabled" type="checkbox" />禁用</label
            ><label
              ><input
                v-model="readonly"
                type="checkbox"
              />只读（适用输入）</label
            ><label><input v-model="invalid" type="checkbox" />错误</label
            ><label><input v-model="busy" type="checkbox" />忙碌</label
            ><label><input v-model="empty" type="checkbox" />空数据</label
            ><label
              ><input v-model="reducedMotion" type="checkbox" />减少动态</label
            ><label
              ><input v-model="highContrast" type="checkbox" />高对比</label
            ><label
              >主题<select v-model="theme">
                <option value="system">跟随系统</option>
                <option value="light">浅色</option>
                <option value="dark">深色</option>
              </select></label
            >
          </div>
          <UiSurface class="foundation-example">
            <UiStack v-if="family === 'U01'"
              ><UiText as="h2">标题文字</UiText><UiText>正文说明</UiText
              ><UiText tone="muted">辅助说明</UiText
              ><UiText truncate style="width: 140px"
                >这是一段可以截断的较长文字内容</UiText
              ><UiIcon label="示例图标"><Sparkles /></UiIcon
            ></UiStack>
            <UiGrid v-else-if="family === 'U02'"
              ><UiSurface
                ><UiStack direction="row"
                  ><span>左侧</span><UiDivider vertical /><span
                    >右侧</span
                  ></UiStack
                ></UiSurface
              ><UiSurface>自适应网格<UiDivider />分隔内容</UiSurface></UiGrid
            >
            <UiStack v-else-if="family === 'U03'" direction="row" wrap
              ><UiButton
                :disabled="disabled"
                :busy="busy"
                variant="primary"
                @click="log('主按钮')"
                >主要操作</UiButton
              ><UiButton :disabled="disabled">次要操作</UiButton
              ><UiButton :disabled="disabled" variant="danger"
                >危险操作</UiButton
              ><UiButton :disabled="disabled" icon aria-label="添加">+</UiButton
              ><UiLink :disabled="disabled" @activate="log('链接意图')"
                >查看说明</UiLink
              ></UiStack
            >
            <UiStack v-else-if="family === 'U04'"
              ><UiField
                label="单行文字"
                description="输入任意文字"
                :error="invalid ? '请修正文字' : undefined"
                ><UiTextInput
                  v-model="text"
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
              ><UiField label="多行文字"
                ><UiTextInput
                  v-model="text"
                  multiline
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
            ></UiStack>
            <UiField
              v-else-if="family === 'U05'"
              label="数值"
              :error="invalid ? '数值不适用' : undefined"
              ><UiNumberInput
                v-model="number"
                :min="0"
                :max="100"
                :step="5"
                unit="单位"
                :disabled="disabled"
                :readonly="readonly"
            /></UiField>
            <UiField v-else-if="family === 'U06'" label="搜索"
              ><UiSearchInput
                v-model="query"
                :delay="200"
                :disabled="disabled"
                @search="log(`搜索 ${$event}`)"
            /></UiField>
            <UiStack v-else-if="family === 'U07'"
              ><UiField label="复选框"
                ><UiCheckbox
                  v-model="checked"
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
              ><UiButton @click="checked = 'indeterminate'">混合状态</UiButton
              ><UiRadioGroup
                v-model="selected"
                :options="options"
                label="单选组"
                :disabled="disabled"
                :readonly="readonly"
            /></UiStack>
            <UiField v-else-if="family === 'U08'" label="开关"
              ><UiSwitch v-model="on" :disabled="disabled" :readonly="readonly"
            /></UiField>
            <UiStack v-else-if="family === 'U09'"
              ><UiField label="选择器"
                ><UiSelect
                  v-model="selected"
                  :options="empty ? [] : options"
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
              ><UiField label="组合框"
                ><UiCombobox
                  v-model="selected"
                  v-model:query="query"
                  :options="
                    empty ? [] : options.filter((o) => o.label.includes(query))
                  "
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
            ></UiStack>
            <UiField v-else-if="family === 'U10'" label="滑块"
              ><UiSlider
                :model-value="number ?? 0"
                @update:model-value="number = $event"
                :disabled="disabled"
                :readonly="readonly"
              /><output>{{ number }}</output></UiField
            >
            <UiStack v-else-if="family === 'U11'"
              ><UiField label="日期"
                ><UiDateInput
                  v-model="date"
                  min="2026-01-01"
                  max="2026-12-31"
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
              ><UiField label="时间"
                ><UiTimeInput
                  v-model="time"
                  :disabled="disabled"
                  :readonly="readonly" /></UiField
            ></UiStack>
            <UiField
              v-else-if="family === 'U12'"
              label="必填字段"
              required
              description="说明始终与输入关联"
              :error="invalid ? '请填写有效内容' : undefined"
              ><UiTextInput
                v-model="text"
                :disabled="disabled"
                :readonly="readonly"
            /></UiField>
            <UiList
              v-else-if="family === 'U13'"
              :items="empty ? [] : items"
              v-model="selection"
              multiple
              :disabled="disabled"
              label="集合选择"
              @activate="log(`激活 ${$event}`)"
            />
            <UiTree
              v-else-if="family === 'U14'"
              :items="empty ? [] : nodes"
              v-model="selection"
              v-model:expanded="expanded"
              :disabled="disabled"
              label="层级集合"
              @load="load"
              @activate="log(`激活 ${$event}`)"
            />
            <UiTable
              v-else-if="family === 'U15'"
              :rows="rows"
              :columns="[
                { key: 'label', label: '名称', sortable: true },
                { key: 'id', label: '编号' },
              ]"
              v-model="selection"
              v-model:sort="sort"
              selection="multiple"
              label="数据表格"
              :disabled="disabled"
              v-model:page="page"
              :page-count="3"
            />
            <UiTabs
              v-else-if="family === 'U16'"
              v-model="tab"
              :items="items"
              label="视图选择"
              ><template #default="{ item }"
                >{{ item.label }} 的内容</template
              ></UiTabs
            >
            <UiStack v-else-if="family === 'U17'"
              ><UiNavigation
                :items="items"
                v-model="tab"
                label="示例导航" /><UiNavigation
                :items="items"
                v-model="tab"
                label="面包屑"
                breadcrumbs
            /></UiStack>
            <UiCollapsible
              v-else-if="family === 'U18'"
              v-model:open="open"
              label="展开内容"
              :disabled="disabled"
              ><UiTextInput v-model="text" aria-label="折叠区文字"
            /></UiCollapsible>
            <UiSplitPane
              v-else-if="family === 'U19'"
              v-model="ratio"
              style="height: 220px"
              ><template #first>第一区域</template
              ><template #second>第二区域</template></UiSplitPane
            >
            <UiStack v-else-if="family === 'U20'"
              ><UiToolbar label="编辑工具栏"
                ><UiToolbarButton :disabled="disabled" @click="log('添加')"
                  >添加</UiToolbarButton
                ><UiToolbarButton :disabled="disabled" @click="log('复制')"
                  >复制</UiToolbarButton
                ></UiToolbar
              ><UiMenu
                v-model:open="menu"
                label="示例菜单"
                :items="items"
                @select="log(`菜单 ${$event}`)"
                ><template #trigger
                  ><UiButton :disabled="disabled">打开菜单</UiButton></template
                ></UiMenu
              ></UiStack
            >
            <UiStack v-else-if="family === 'U21'" direction="row"
              ><UiTooltip text="辅助提示内容"
                ><UiButton :disabled="disabled">查看提示</UiButton></UiTooltip
              ><UiPopover v-model:open="popover" label="锚定浮层"
                ><template #trigger
                  ><UiButton :disabled="disabled">打开浮层</UiButton></template
                ><UiField label="浮层文字"
                  ><UiTextInput v-model="text" /></UiField></UiPopover
            ></UiStack>
            <template v-else-if="family === 'U22'"
              ><UiButton :disabled="disabled" @click="open = true"
                >打开对话框</UiButton
              ><UiDialog
                v-model:open="open"
                title="示例对话框"
                description="取消或确认只发送通用事件。"
                ><UiField label="对话框选择"
                  ><UiSelect v-model="selected" :options="options" /></UiField
                ><template #actions
                  ><UiButton
                    @click="
                      open = false;
                      log('取消');
                    "
                    >取消</UiButton
                  ><UiButton
                    variant="primary"
                    @click="
                      open = false;
                      log('确认');
                    "
                    >确认</UiButton
                  ></template
                ></UiDialog
              ></template
            >
            <UiStack v-else-if="family === 'U23'"
              ><UiProgress
                :value="busy ? undefined : (number ?? 0)"
                label="示例进度" /><UiSpinner /><UiSteps
                :items="items"
                :current="selected" /><UiSlider
                :model-value="number ?? 0"
                @update:model-value="number = $event"
                aria-label="进度数值"
            /></UiStack>
            <UiStack v-else-if="family === 'U24'"
              ><UiBadge tone="success">成功</UiBadge
              ><UiMessage tone="warning" dismissible @dismiss="log('关闭消息')"
                >示例提示，由调用方提供文字。</UiMessage
              ><UiButton
                @click="
                  toasts.push({ id: String(Date.now()), text: '示例通知' })
                "
                >显示通知</UiButton
              ><UiToasts
                :items="toasts"
                @dismiss="toasts = toasts.filter((t) => t.id !== $event)"
            /></UiStack>
            <UiStack v-else-if="family === 'U25'"
              ><UiSkeleton /><UiEmptyState
                :state="busy ? 'loading' : invalid ? 'error' : 'empty'"
                :title="busy ? '加载中' : invalid ? '加载失败' : '暂无内容'"
                retry
                @retry="log('重试意图')"
            /></UiStack>
            <UiStack v-else-if="family === 'U26'"
              ><UiChart
                v-model="visibleSeries"
                label="数据趋势"
                :categories="empty ? [] : ['一', '二', '三', '四']"
                :series="[
                  { id: 'a', label: '系列 A', values: [4, 8, 5, 12] },
                  { id: 'b', label: '系列 B', values: [2, -3, null, 6] },
                ]" /><UiChart
                v-model="visibleSeries"
                kind="bar"
                label="数据对比"
                :categories="empty ? [] : ['一', '二', '三', '四']"
                :series="[
                  { id: 'a', label: '系列 A', values: [4, 8, 5, 12] },
                  { id: 'b', label: '系列 B', values: [2, -3, null, 6] },
                ]"
            /></UiStack>
          </UiSurface>
          <p role="status" class="foundation-events">{{ events }}</p>
        </main>
      </div>
    </section>
  </UiProvider>
</template>
<style>
.foundation-gallery {
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
  padding: 20px;
  min-height: 100vh;
  box-sizing: border-box;
}
.foundation-gallery h1 {
  font-size: 24px;
  margin: 0 0 8px;
}
.foundation-gallery h2 {
  font-size: 20px;
  margin-top: 0;
}
.foundation-gallery header p,
.foundation-gallery main > p {
  color: var(--rss-color-text-muted);
  font-size: 12px;
}
.foundation-layout {
  display: grid;
  grid-template-columns: 190px minmax(0, 1fr);
  gap: 24px;
  margin-top: 24px;
}
.foundation-gallery .ui-navigation button {
  min-height: 28px;
  font-size: 12px;
}
.foundation-switches {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 12px;
  margin: 16px 0;
}
.foundation-switches label {
  display: flex;
  gap: 4px;
  align-items: center;
}
.foundation-example {
  min-height: 180px;
}
.foundation-events {
  padding: 12px;
  border-top: 1px solid var(--rss-color-border);
}
@media (max-width: 650px) {
  .foundation-layout {
    grid-template-columns: 1fr;
  }
  .foundation-layout > nav ol {
    display: flex;
    flex-wrap: wrap;
  }
}
</style>
