import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SystemSettings from "./SystemSettings.vue";

describe("system settings preview", () => {
  it("filters settings, restores search and focus on return, and does not invent system state", async () => {
    const wrapper = mount(SystemSettings, { attachTo: document.body });
    await wrapper.get("input").setValue("网络");
    const entry = wrapper.get(".settings-list button");
    expect(wrapper.findAll(".settings-list button")).toHaveLength(1);
    await entry.trigger("click");
    expect(document.activeElement).toBe(
      wrapper.get(".detail-title h2").element,
    );
    expect(
      wrapper
        .findAll(".setting-row")
        .every((row) => row.text().includes("未读取")),
    ).toBe(true);
    expect(wrapper.text()).toContain("此处不会读取或修改你的系统配置");
    await wrapper.get(".settings-back").trigger("click");
    expect(wrapper.get("input").element.value).toBe("网络");
    expect(document.activeElement).toBe(entry.element);
    await wrapper.get("input").setValue("不存在的设置");
    expect(wrapper.get('[role="status"]').text()).toContain("没有匹配");
    await wrapper.get(".computer-return").trigger("click");
    expect(wrapper.emitted("home")).toHaveLength(1);
    wrapper.unmount();
  });
});
