import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import {
  AppShell,
  NavigationList,
  SplitPane,
  MessageStream,
  MessageComposer,
  StatusList,
} from "../src";

describe("text-only presentation", () => {
  it("renders untrusted content as text without actions or links", () => {
    const text =
      "<img src=x onerror=alert(1)><script>alert(1)</script> javascript:alert(1) \x1b]52;c;payload\x07";
    const stream = mount(MessageStream, {
      props: {
        items: [
          { id: "a", kind: "assistant", text },
          { id: "r", kind: "reasoning", text },
          { id: "u", kind: "user", text },
        ],
      },
    });
    expect(stream.findAll("script,img,a,iframe")).toHaveLength(0);
    expect(stream.find("pre").text()).toBe(text);
    expect(stream.find("details").element.open).toBe(false);
    expect(stream.emitted()).toEqual({});
    const status = mount(StatusList, {
      props: {
        items: [{ id: "s", label: text, message: text, tone: "danger" }],
      },
    });
    expect(status.findAll("script,img,a")).toHaveLength(0);
    expect(status.text()).toContain(text);
  });
  it("handles empty and updated streams", async () => {
    const stream = mount(MessageStream, { props: { items: [] } });
    expect(stream.findAll("pre")).toHaveLength(0);
    await stream.setProps({
      items: [{ id: "a", kind: "assistant", text: "new output" }],
    });
    expect(stream.text()).toContain("new output");
  });
});

describe("composer intent", () => {
  it("emits a trimmed submission without clearing controlled text", async () => {
    const wrapper = mount(MessageComposer, {
      props: { modelValue: "  hello  " },
    });
    await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
    expect(wrapper.emitted("submit")).toEqual([["hello"]]);
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    await wrapper.get("textarea").setValue("next");
    expect(wrapper.emitted("update:modelValue")).toEqual([["next"]]);
  });
  it("does not submit multiline, IME or legacy IME Enter", async () => {
    const wrapper = mount(MessageComposer, { props: { modelValue: "hello" } });
    for (const event of [
      { shiftKey: true },
      { isComposing: true },
      { keyCode: 229 },
    ]) {
      await wrapper
        .get("textarea")
        .trigger("keydown", { key: "Enter", ...event });
    }
    expect(wrapper.emitted("submit")).toBeUndefined();
  });
  it.each([{ modelValue: "  " }, { disabled: true }, { busy: true }])(
    "blocks submission: %j",
    async (props) => {
      const wrapper = mount(MessageComposer, {
        props: { modelValue: "hello", ...props },
      });
      await wrapper.get("form").trigger("submit");
      await wrapper.get("textarea").trigger("keydown", { key: "Enter" });
      expect(wrapper.emitted("submit")).toBeUndefined();
    },
  );
  it("collapse prevents submission; cancel requires explicit availability", async () => {
    const wrapper = mount(MessageComposer, {
      props: { modelValue: "hello", busy: true, canCancel: true },
    });
    await wrapper.get('[data-action="cancel"]').trigger("click");
    expect(wrapper.emitted("cancel")).toEqual([[]]);
    await wrapper.setProps({ canCancel: false, busy: false });
    expect(wrapper.find('[data-action="cancel"]').exists()).toBe(false);
    await wrapper.get("[aria-expanded]").trigger("click");
    expect(wrapper.find("textarea").exists()).toBe(false);
    await wrapper.get("form").trigger("submit");
    expect(wrapper.emitted("submit")).toBeUndefined();
  });
});

it("navigation emits only an enabled id and reflects active state", async () => {
  const wrapper = mount(NavigationList, {
    props: {
      activeId: "a",
      items: [
        { id: "a", label: "Alpha" },
        { id: "b", label: "Beta", disabled: true },
      ],
    },
  });
  expect(wrapper.get('[aria-current="page"]').text()).toBe("Alpha");
  await wrapper.findAll("button")[0].trigger("click");
  await wrapper.findAll("button")[1].trigger("click");
  expect(wrapper.emitted("select")).toEqual([["a"]]);
});

it("lays out all named slots without querying a host", () => {
  const fetchSpy = vi
    .spyOn(globalThis, "fetch")
    .mockRejectedValue(new Error("unexpected network"));
  const wrapper = mount(AppShell, {
    slots: {
      header: "Header",
      navigation: "Navigation",
      default: "Content",
      status: "Status",
    },
  });
  expect(wrapper.get("header").text()).toBe("Header");
  expect(wrapper.get("aside").text()).toBe("Navigation");
  expect(wrapper.get("main").text()).toBe("Content");
  expect(wrapper.get("footer").text()).toBe("Status");
  expect(fetchSpy).not.toHaveBeenCalled();
});

it("split keyboard bounds, dynamic minimum and pointer capture agree with ARIA", async () => {
  const wrapper = mount(SplitPane, {
    props: { initialTopRatio: 0.5, minRatio: 0.15 },
    slots: { top: "Top", bottom: "Bottom" },
  });
  const divider = wrapper.get('[role="separator"]');
  await divider.trigger("keydown", { key: "Home" });
  expect(divider.attributes("aria-valuenow")).toBe("15");
  await divider.trigger("keydown", { key: "End" });
  expect(divider.attributes("aria-valuenow")).toBe("85");
  await divider.trigger("keydown", { key: "ArrowUp" });
  expect(divider.attributes("aria-valuenow")).toBe("83");
  await wrapper.setProps({ minRatio: 0.3 });
  expect(divider.attributes("aria-valuenow")).toBe("70");
  expect(divider.attributes("aria-valuemax")).toBe("70");
  const capture = vi.fn();
  Object.defineProperty(divider.element, "setPointerCapture", {
    value: capture,
  });
  vi.spyOn(wrapper.element, "getBoundingClientRect").mockReturnValue({
    top: 100,
    height: 100,
  } as DOMRect);
  await divider.trigger("pointerdown", { pointerId: 1, button: 0 });
  expect(capture).toHaveBeenCalledWith(1);
  await divider.trigger("pointermove", { pointerId: 1, clientY: 150 });
  expect(divider.attributes("aria-valuenow")).toBe("50");
  await divider.trigger("lostpointercapture");
  await divider.trigger("pointermove", { pointerId: 1, clientY: 200 });
  expect(divider.attributes("aria-valuenow")).toBe("50");
  expect(wrapper.emitted("resize")?.at(-1)).toEqual([0.5]);
});
