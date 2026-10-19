import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import type { PageNavigation } from "../workspace/page-navigation";
import SystemSettings from "./SystemSettings.vue";
it("filters the directory, restores search and focus, and keeps unknown system state explicit", async () => {
  const w = mount(SystemSettings, { attachTo: document.body });
  const navigation = w.emitted("navigation")![0][0] as PageNavigation;
  try {
    await w.get("input").setValue("网络");
    expect(w.findAll(".settings-overview-grid button")).toHaveLength(1);
    await w.get('[data-setting="network"]').trigger("click");
    expect(document.activeElement).toBe(w.get("h2").element);
    expect(
      w.findAll(".setting-row").every((e) => e.text().includes("未读取")),
    ).toBe(true);
    expect(w.text()).toContain("此处不会读取或修改你的系统配置");
    await w.get(".settings-back").trigger("click");
    expect(w.get("input").element.value).toBe("网络");
    expect(document.activeElement).toBe(
      w.get('[data-setting="network"]').element,
    );
    await w.get("input").setValue("没有这个设置");
    expect(w.get('[role="status"]').text()).toContain("没有匹配");
    await w.get(".computer-return").trigger("click");
    expect(w.emitted("home")).toHaveLength(1);
  } finally {
    w.unmount();
  }
});
it("keeps sleep under power, exposes clickable ancestors, and resets to overview", async () => {
  const w = mount(SystemSettings);
  const navigation = w.emitted("navigation")![0][0] as PageNavigation;
  try {
    expect(w.find('[data-setting="lock"]').exists()).toBe(false);
    expect(navigation.sections.some((e) => e.id === "lock")).toBe(false);
    await w.get('[data-setting="power"]').trigger("click");
    await w.get('[data-setting="lock"]').trigger("click");
    expect(navigation.section).toBe("power");
    expect(navigation.path.map((e) => e.id)).toEqual(["power", "lock"]);
    await navigation.openSection("power");
    expect(w.find('[data-setting="lock"]').exists()).toBe(true);
    navigation.reset();
    await w.vm.$nextTick();
    expect(w.find(".settings-overview-grid").exists()).toBe(true);
    expect(navigation.path).toEqual([]);
  } finally {
    w.unmount();
  }
});
it("restores focus to search when a directory result is no longer present", async () => {
  const w = mount(SystemSettings, { attachTo: document.body });
  const navigation = w.emitted("navigation")![0][0] as PageNavigation;
  try {
    await w.get('[data-setting="sound"]').trigger("click");
    navigation.setQuery!("不存在");
    await w.get(".settings-back").trigger("click");
    expect(document.activeElement).toBe(w.get("input").element);
  } finally {
    w.unmount();
  }
});
