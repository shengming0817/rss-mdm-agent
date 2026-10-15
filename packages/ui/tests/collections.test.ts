import { mount } from "@vue/test-utils";
import { describe, it, expect } from "vitest";
import {
  UiTable,
  UiList,
  UiTree,
  UiTabs,
  UiCollapsible,
  UiChart,
  UiEmptyState,
  UiProgress,
  UiDateTimeInput,
} from "../src";
const items = [
  { id: "a", label: "Alpha" },
  { id: "b", label: "Beta", disabled: true },
];
describe("controlled collections and feedback", () => {
  it("table emits sorting and pagination intents without changing caller data", async () => {
    const wrapper = mount(UiTable, {
      props: {
        label: "Rows",
        rows: items,
        columns: [{ key: "label", label: "Name", sortable: true }],
        modelValue: [],
        selection: "multiple",
        page: 1,
        pageCount: 2,
      },
    });
    await wrapper.get('input[type="checkbox"]').setValue(true);
    expect(wrapper.emitted("update:modelValue")).toEqual([[["a"]]]);
    await wrapper.get("th button").trigger("click");
    expect(wrapper.emitted("update:sort")).toEqual([
      [{ key: "label", direction: "ascending" }],
    ]);
    expect(wrapper.findAll("tbody tr")[0].text()).toContain("Alpha");
    await wrapper.findAll("nav button")[1].trigger("click");
    expect(wrapper.emitted("update:page")).toEqual([[2]]);
    expect(wrapper.findAll("input")[1].attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });
  it("separates collection activation from selection and rejects disabled activation", async () => {
    const wrapper = mount(UiList, {
      props: { items, modelValue: [], label: "List" },
    });
    await wrapper.findAll('[role="option"]')[0].trigger("dblclick");
    await wrapper.findAll('[role="option"]')[1].trigger("dblclick");
    expect(wrapper.emitted("activate")).toEqual([["a"]]);
    wrapper.unmount();
  });
  it("exposes explicit async tree loading without doing IO", async () => {
    const wrapper = mount(UiTree, {
      props: {
        items: [{ id: "a", label: "Parent", hasChildren: true }],
        modelValue: [],
        expanded: [],
        label: "Tree",
      },
    });
    const item = wrapper.get('[role="treeitem"]');
    await item.trigger("keydown", { key: "ArrowRight" });
    expect(wrapper.emitted("load")).toEqual([["a"]]);
    expect(wrapper.emitted("update:expanded")?.at(-1)).toEqual([["a"]]);
    wrapper.unmount();
  });
  it("retains inactive content but removes it from focus and accessibility", async () => {
    const tabs = mount(UiTabs, {
      props: { items, modelValue: "a", label: "Tabs" },
      slots: { default: '<input value="draft">' },
    });
    const hidden = tabs.findAll("[hidden]");
    expect(hidden.length).toBeGreaterThan(0);
    expect(hidden.some((node) => node.find("input").exists())).toBe(true);
    const collapse = mount(UiCollapsible, {
      props: { open: false, label: "More" },
      slots: { default: "<button>Hidden</button>" },
    });
    expect(collapse.get("[hidden]").get("button").text()).toBe("Hidden");
    await collapse.get("[aria-expanded]").trigger("click");
    expect(collapse.emitted("update:open")).toEqual([[true]]);
    tabs.unmount();
    collapse.unmount();
  });
  it("charts preserve missing and negative values in an accessible table", () => {
    const wrapper = mount(UiChart, {
      props: {
        label: "Chart",
        categories: ["one", "two", "three"],
        series: [{ id: "s", label: "Series", values: [-4, null, 8] }],
        modelValue: ["s"],
      },
    });
    expect(wrapper.get("svg").attributes("role")).toBe("img");
    expect(wrapper.get("path").attributes("d")).not.toContain("NaN");
    expect(wrapper.get("table").text()).toContain("-4");
    expect(wrapper.get("table").text()).toContain("—");
    wrapper.unmount();
  });
  it("progress and empty state do not infer task completion", async () => {
    const progress = mount(UiProgress, { props: { label: "Progress" } });
    expect(progress.attributes("value")).toBeUndefined();
    await progress.setProps({ value: 100 });
    expect(progress.attributes("value")).toBe("100");
    expect(progress.emitted()).toEqual({});
    const empty = mount(UiEmptyState, {
      props: { state: "error", title: "Failed", retry: true },
    });
    await empty.get("button").trigger("click");
    expect(empty.emitted("retry")).toEqual([[]]);
    progress.unmount();
    empty.unmount();
  });
  it("datetime segments have separate labels and never convert timezones", () => {
    const wrapper = mount(UiDateTimeInput, {
      props: { modelValue: "2026-10-15T09:30" },
    });
    const ids = wrapper.findAll("[id]").map((node) => node.attributes("id"));
    expect(new Set(ids).size).toBe(ids.length);
    expect(wrapper.text()).toContain("2026");
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    wrapper.unmount();
  });
});
