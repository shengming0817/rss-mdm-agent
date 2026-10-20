import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
const native = vi.hoisted(() => ({
  available: true,
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
  close: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => native.available }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => native }));
import WindowControls from "./WindowControls.vue";
it("controls the current window and reports failures without exposing native errors", async () => {
  const wrapper = mount(WindowControls);
  try {
    await wrapper.get('[aria-label="最小化窗口"]').trigger("click");
    await wrapper.get('[aria-label="最大化或还原窗口"]').trigger("click");
    expect(native.minimize).toHaveBeenCalledOnce();
    expect(native.toggleMaximize).toHaveBeenCalledOnce();
    native.close.mockRejectedValueOnce(new Error("private native details"));
    await wrapper.get('[aria-label="关闭窗口"]').trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe("窗口操作未完成，请重试");
    await wrapper.get('[aria-label="关闭窗口"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  } finally {
    wrapper.unmount();
  }
});
it("does not present inert system buttons in a browser preview", () => {
  native.available = false;
  const wrapper = mount(WindowControls);
  try {
    expect(wrapper.find("button").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    native.available = true;
  }
});
