import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AppShell from "../src/components/AppShell.vue";

let resizeWindow: (width: number) => void;
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    new DOMRect(0, 0, 1100, 760),
  );
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        resizeWindow = (width) =>
          callback(
            [{ contentRect: { width } } as ResizeObserverEntry],
            this as unknown as ResizeObserver,
          );
      }
      observe() {}
      disconnect() {}
    },
  );
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
const shell = () =>
  mount(AppShell, {
    attachTo: document.body,
    props: {
      navigationEnabled: true,
      contentMode: "page",
      navigationKey: "home",
    },
    slots: {
      navigation: "导航",
      default: '<textarea aria-label="页面草稿">保留内容</textarea>',
      header: "<h1>首页</h1>",
    },
  });

it("hides desktop navigation without losing page state or the chosen width", async () => {
  const wrapper = shell();
  try {
    await nextTick();
    const draft = wrapper.get("textarea").element;
    await wrapper
      .get('[role="separator"]')
      .trigger("keydown", { key: "ArrowRight" });
    await wrapper.get('[aria-label="收起侧边栏"]').trigger("click");
    expect(wrapper.get("aside").isVisible()).toBe(false);
    expect(wrapper.get("textarea").element).toBe(draft);
    expect(
      wrapper.get('[aria-label="展开侧边栏"]').attributes("aria-expanded"),
    ).toBe("false");
    expect(document.activeElement).toBe(
      wrapper.get(".navigation-trigger").element,
    );
    await wrapper.setProps({ navigationKey: "software" });
    expect(wrapper.get("aside").isVisible()).toBe(false);
    await wrapper.get('[aria-label="展开侧边栏"]').trigger("click");
    expect(wrapper.get("aside").isVisible()).toBe(true);
    expect(wrapper.get("aside").attributes("style")).toContain("256px");
  } finally {
    wrapper.unmount();
  }
});

it("bounds pointer and keyboard resizing and releases a cancelled drag", async () => {
  const wrapper = shell();
  try {
    await nextTick();
    const handle = wrapper.get('[role="separator"]');
    let captured = false;
    const release = vi.fn(() => {
      captured = false;
    });
    Object.assign(handle.element, {
      setPointerCapture: vi.fn(() => {
        captured = true;
      }),
      hasPointerCapture: () => captured,
      releasePointerCapture: release,
    });
    await handle.trigger("pointerdown", {
      button: 0,
      pointerId: 1,
      clientX: 240,
    });
    await handle.trigger("pointermove", { pointerId: 2, clientX: 300 });
    expect(handle.attributes("aria-valuenow")).toBe("240");
    await handle.trigger("pointermove", { pointerId: 1, clientX: 900 });
    expect(handle.attributes("aria-valuenow")).toBe("360");
    await handle.trigger("pointercancel", { pointerId: 1 });
    expect(release).toHaveBeenCalledWith(1);
    await handle.trigger("pointermove", { pointerId: 1, clientX: 0 });
    expect(handle.attributes("aria-valuenow")).toBe("360");
    await handle.trigger("keydown", { key: "Home" });
    await handle.trigger("keydown", { key: "ArrowLeft" });
    expect(handle.attributes("aria-valuenow")).toBe("200");
    await handle.trigger("keydown", { key: "End" });
    expect(handle.attributes("aria-valuenow")).toBe("360");
    await handle.trigger("dblclick");
    expect(handle.attributes("aria-valuenow")).toBe("240");
    await handle.trigger("pointerdown", {
      button: 0,
      pointerId: 3,
      clientX: 240,
    });
    await wrapper.get(".navigation-trigger").trigger("click");
    expect(release).toHaveBeenCalledWith(3);
  } finally {
    wrapper.unmount();
  }
});

it("uses a drawer on narrow windows and restores the desktop preference", async () => {
  const wrapper = shell();
  try {
    await nextTick();
    await wrapper.get(".navigation-trigger").trigger("click");
    resizeWindow(700);
    await nextTick();
    expect(wrapper.find(".navigation-panel").exists()).toBe(false);
    const trigger = wrapper.get('[aria-label="打开主导航"]');
    await trigger.trigger("click");
    expect(trigger.attributes("aria-expanded")).toBe("true");
    await wrapper.setProps({ navigationKey: "tools" });
    expect(trigger.attributes("aria-expanded")).toBe("false");
    resizeWindow(1100);
    await nextTick();
    expect(wrapper.get("aside").isVisible()).toBe(false);
    await wrapper.get('[aria-label="展开侧边栏"]').trigger("click");
    expect(wrapper.get("aside").isVisible()).toBe(true);
  } finally {
    wrapper.unmount();
  }
});
