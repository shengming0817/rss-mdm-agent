import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { h, ref, nextTick } from "vue";
import {
  UiButton,
  UiLink,
  UiField,
  UiTextInput,
  UiCheckbox,
  UiSwitch,
  UiProvider,
  UiNumberInput,
  UiSearchInput,
} from "../src";

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
    const wrapper = mount(
      {
        setup: () => () =>
          h(UiField, { ref: field, label: "标题", error: "请输入内容" }, () =>
            h(UiTextInput, { modelValue: "" }),
          ),
      },
      { attachTo: document.body },
    );
    const input = wrapper.get("input");
    expect(wrapper.get("label").attributes("for")).toBe(input.attributes("id"));
    expect(input.attributes("aria-describedby")).toBe(
      wrapper.get('[role="alert"]').attributes("id"),
    );
    expect(input.attributes("aria-invalid")).toBe("true");
    field.value?.focus();
    expect(document.activeElement).toBe(input.element);
    wrapper.unmount();
  });
  it("does not mutate readonly switches or mixed checkboxes", async () => {
    for (const component of [UiCheckbox, UiSwitch]) {
      const wrapper = mount(component, {
        props: { modelValue: false, readonly: true },
      });
      await wrapper.get("button").trigger("click");
      expect(wrapper.emitted("update:modelValue")).toBeUndefined();
      wrapper.unmount();
    }
    const mixed = mount(UiCheckbox, { props: { modelValue: "indeterminate" } });
    expect(mixed.get("button").attributes("aria-checked")).toBe("mixed");
    mixed.unmount();
  });
  it("cancels pending search callbacks on disposal", async () => {
    const wrapper = mount(UiSearchInput, {
      props: { modelValue: "", delay: 10 },
    });
    await wrapper.get("input").setValue("value");
    wrapper.unmount();
    await new Promise((resolve) => setTimeout(resolve, 25));
    expect(wrapper.emitted("search")).toBeUndefined();
  });
  it("has independent platform and portal roots", async () => {
    const wrapper = mount({
      render: () =>
        h("div", [
          h(UiProvider, { platform: "macos", theme: "dark" }),
          h(UiProvider, { platform: "windows", theme: "light" }),
        ]),
    });
    await nextTick();
    expect(wrapper.findAll(".ui-overlays")).toHaveLength(2);
    expect(
      wrapper.findAll("[data-platform]").map((n) => n.attributes("data-theme")),
    ).toEqual(["dark", "light"]);
    wrapper.unmount();
  });
  it("retains an empty numeric input", () => {
    const wrapper = mount(UiNumberInput, { props: { modelValue: null } });
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe("");
    wrapper.unmount();
  });
});

import { flushPromises } from "@vue/test-utils";
import {
  UiDateInput,
  UiTimeInput,
  UiDateTimeInput,
  UiRadioGroup,
  UiSlider,
  UiList,
  UiChart,
  UiDialog,
} from "../src";
import { DatePickerRoot, TimeFieldRoot } from "reka-ui";

it("keeps empty date and time roots explicitly controlled", () => {
  for (const [component, root] of [
    [UiDateInput, DatePickerRoot],
    [UiTimeInput, TimeFieldRoot],
  ] as const) {
    const wrapper = mount(component, { props: { modelValue: "" } });
    expect(wrapper.getComponent(root).props("modelValue")).toBeNull();
    wrapper.unmount();
  }
});
it("does not accept completed datetime edits until the caller updates its value", async () => {
  const wrapper = mount(UiDateTimeInput, {
    props: { modelValue: "2026-10-15T09:30" },
    attachTo: document.body,
  });
  const day = wrapper.get('[data-reka-date-field-segment="day"]');
  await day.trigger("keydown", { key: "ArrowUp" });
  await flushPromises();
  expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([
    "2026-10-16T09:30:00",
  ]);
  expect(day.text()).toBe("15");
  await wrapper.setProps({ modelValue: "2026-10-20T10:00" });
  expect(day.text()).toBe("20");
  await wrapper.setProps({ modelValue: "" });
  expect(wrapper.text()).not.toContain("2026");
  wrapper.unmount();
});
it("does not select an initially empty readonly radio group", async () => {
  const wrapper = mount(UiRadioGroup, {
    props: {
      label: "Choice",
      readonly: true,
      options: [{ value: "a", label: "Alpha" }],
    },
  });
  await wrapper.get('[role="radio"]').trigger("click");
  expect(wrapper.get('[role="radio"]').attributes("aria-checked")).toBe(
    "false",
  );
  expect(wrapper.emitted("update:modelValue")).toBeUndefined();
  wrapper.unmount();
});
it("names and focuses a non-labelable field control", async () => {
  const wrapper = mount(
    {
      render: () =>
        h(UiField, { label: "Level" }, () => h(UiSlider, { modelValue: 50 })),
    },
    { attachTo: document.body },
  );
  expect(wrapper.get('[role="slider"]').attributes("aria-labelledby")).toBe(
    wrapper.get("label").attributes("id"),
  );
  await wrapper.get("label").trigger("click");
  expect(document.activeElement).toBe(wrapper.get('[role="slider"]').element);
  wrapper.unmount();
});
it("names the actual listbox rather than its wrapper", () => {
  const wrapper = mount(UiList, {
    props: { label: "Choices", modelValue: [], items: [] },
  });
  expect(wrapper.get('[role="listbox"]').attributes("aria-label")).toBe(
    "Choices",
  );
  wrapper.unmount();
});
it("keeps category labels for reserved series IDs and draws isolated samples", () => {
  const wrapper = mount(UiChart, {
    props: {
      label: "Chart",
      categories: ["one", "two", "three"],
      series: [{ id: "label", label: "Reserved", values: [8, null, 4] }],
      modelValue: ["label"],
    },
  });
  expect(
    wrapper
      .findAll("tbody tr")
      .map((row) => row.findAll("td").map((td) => td.text())),
  ).toEqual([
    ["one", "8"],
    ["two", "—"],
    ["three", "4"],
  ]);
  expect(wrapper.findAll("circle")).toHaveLength(2);
  wrapper.unmount();
});
it("forwards caller identity and styling to the actual dialog", async () => {
  const wrapper = mount(UiDialog, {
    props: { open: true, title: "Panel", presentation: "drawer" },
    attrs: { id: "external-panel", class: "consumer-panel" },
    attachTo: document.body,
  });
  await flushPromises();
  const dialog = document.querySelector("#external-panel")!;
  expect(dialog.getAttribute("role")).toBe("dialog");
  expect(dialog.classList.contains("consumer-panel")).toBe(true);
  wrapper.unmount();
});

import { vi } from "vitest";
import { UiToasts, UiSplitPane } from "../src";
it("removes toast close timers when a toast is removed or its owner is disposed", async () => {
  vi.useFakeTimers();
  const wrapper = mount(UiToasts, {
    props: { items: [{ id: "one", text: "Hello" }] },
    attachTo: document.body,
  });
  try {
    await flushPromises();
    await vi.advanceTimersByTimeAsync(0);
    await wrapper.setProps({ items: [] });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
    await wrapper.setProps({ items: [{ id: "two", text: "Again" }] });
    await flushPromises();
  } finally {
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});
it("keeps split dragging owned by one pointer and releases it on unmount", async () => {
  const wrapper = mount(UiSplitPane, { props: { modelValue: 0.5 } });
  const handle = wrapper.get('[role="separator"]');
  const capture = vi.fn(),
    release = vi.fn();
  Object.assign(handle.element, {
    setPointerCapture: capture,
    hasPointerCapture: () => true,
    releasePointerCapture: release,
  });
  vi.spyOn(wrapper.element, "getBoundingClientRect").mockReturnValue({
    left: 0,
    width: 100,
  } as DOMRect);
  await handle.trigger("pointerdown", { pointerId: 1, button: 0 });
  await handle.trigger("pointerdown", { pointerId: 2, button: 0 });
  await handle.trigger("pointerup", { pointerId: 2 });
  await handle.trigger("lostpointercapture", { pointerId: 2 });
  await handle.trigger("pointermove", { pointerId: 1, clientX: 60 });
  expect(capture.mock.calls).toEqual([[1]]);
  expect(wrapper.emitted("update:modelValue")?.at(-1)).toEqual([0.6]);
  expect(release).not.toHaveBeenCalled();
  wrapper.unmount();
  expect(release).toHaveBeenCalledWith(1);
});

import { UiPopover } from "../src";
it("uses the popover's supplied accessible name instead of its trigger text", async () => {
  const wrapper = mount(UiPopover, {
    props: { open: true, label: "Named popup" },
    slots: {
      trigger: () => h(UiButton, {}, () => "Launch"),
      default: () => h("input"),
    },
    attachTo: document.body,
  });
  await flushPromises();
  const popup = document.querySelector(
    '[role="dialog"][aria-label="Named popup"]',
  )!;
  expect(
    document.getElementById(popup.getAttribute("aria-labelledby")!)
      ?.textContent,
  ).toBe("Named popup");
  wrapper.unmount();
});
