import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { h, ref, nextTick } from "vue";
import { UiButton, UiLink, UiField, UiTextInput, UiCheckbox, UiSwitch, UiProvider, UiNumberInput, UiSearchInput } from "../src";

describe("foundation input contracts", () => {
  it("blocks busy actions and exposes link intent without navigation", async () => {
    const button = mount(UiButton, { props: { busy: true } });
    expect(button.attributes("disabled")).toBeDefined();
    expect(button.attributes("aria-busy")).toBe("true");
    const link = mount(UiLink);
    await link.trigger("click");
    expect(link.emitted("activate")).toHaveLength(1);
    expect(link.attributes("href")).toBeUndefined();
  });
  it("associates the real input and exposes focus through Field", async () => {
    const field = ref<InstanceType<typeof UiField>>();
    const wrapper = mount({ setup: () => () => h(UiField, { ref: field, label: "标题", error: "请输入内容" }, () => h(UiTextInput, { modelValue: "" })) }, { attachTo: document.body });
    const input = wrapper.get("input");
    expect(wrapper.get("label").attributes("for")).toBe(input.attributes("id"));
    expect(input.attributes("aria-describedby")).toBe(wrapper.get('[role="alert"]').attributes("id"));
    expect(input.attributes("aria-invalid")).toBe("true");
    field.value?.focus();
    expect(document.activeElement).toBe(input.element);
    wrapper.unmount();
  });
  it("does not mutate readonly switches or mixed checkboxes", async () => {
    for (const component of [UiCheckbox, UiSwitch]) {
      const wrapper = mount(component, { props: { modelValue: false, readonly: true } });
      await wrapper.get("button").trigger("click");
      expect(wrapper.emitted("update:modelValue")).toBeUndefined();
      wrapper.unmount();
    }
    const mixed = mount(UiCheckbox, { props: { modelValue: "indeterminate" } });
    expect(mixed.get("button").attributes("aria-checked")).toBe("mixed");
    mixed.unmount();
  });
  it("cancels pending search callbacks on disposal", async () => {
    const wrapper = mount(UiSearchInput, { props: { modelValue: "", delay: 10 } });
    await wrapper.get("input").setValue("value");
    wrapper.unmount();
    await new Promise(resolve => setTimeout(resolve, 25));
    expect(wrapper.emitted("search")).toBeUndefined();
  });
  it("has independent platform and portal roots", async () => {
    const wrapper = mount({ render: () => h("div", [h(UiProvider, { platform: "macos", theme: "dark" }), h(UiProvider, { platform: "windows", theme: "light" })]) });
    await nextTick();
    expect(wrapper.findAll(".ui-overlays")).toHaveLength(2);
    expect(wrapper.findAll("[data-platform]").map(n => n.attributes("data-theme"))).toEqual(["dark", "light"]);
    wrapper.unmount();
  });
  it("retains an empty numeric input", () => {
    const wrapper = mount(UiNumberInput, { props: { modelValue: null } });
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe("");
    wrapper.unmount();
  });
});
