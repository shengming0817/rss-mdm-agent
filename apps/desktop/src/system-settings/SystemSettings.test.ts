import { mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import type { PageNavigation } from "../workspace/page-navigation";
import SystemSettings from "./SystemSettings.vue";
import { createSystemSettings, systemSettingsKey } from "./controller";
import { homeFacts } from "./facts";
import type { CollectedData, QueryDescriptor } from "../self-service/types";

it("ages the same cached facts in the home summary, setting page and AI context without collecting again", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T12:00:00Z"));
  const controller = createSystemSettings(null, null);
  const source: QueryDescriptor = {
    available: true,
    domains: ["hardware.network"],
    inputSchema: {},
    kind: "native",
    origin: "local",
    policy: null,
    reason: null,
    reference: { id: "network", revision: "a".repeat(64) },
    scope: { kind: "device" },
    title: "Network",
  };
  const value: CollectedData = {
    collectedAt: Date.now() / 1000,
    enumerationComplete: true,
    fields: {},
    next: null,
    quality: "complete",
    query: source.reference,
    request: null,
    rows: [{ values: { addresses: ["127.0.0.1"] } }],
    scope: { kind: "device" },
    stale: false,
  };
  controller.state.sources = [source];
  controller.state.facts.set(source.reference.id, { source, value });
  const w = mount(SystemSettings, {
    global: { provide: { [systemSettingsKey as symbol]: controller } },
  });
  try {
    const navigation = w.emitted("navigation")![0][0] as PageNavigation;
    await w.get('[data-setting="network"]').trigger("click");
    expect(homeFacts(controller).networkDetail).toContain("已采集");
    expect(
      navigation.resources?.find((r) => r.id === "network")?.status,
    ).toContain('"stale":false');
    vi.advanceTimersByTime(301000);
    await w.vm.$nextTick();
    expect(homeFacts(controller).networkDetail).toContain("观察已过期");
    expect(w.text()).toContain("已过期");
    expect(
      navigation.resources?.find((r) => r.id === "network")?.status,
    ).toContain('"stale":true');
    expect(
      controller.state.facts.get(source.reference.id)?.value.collectedAt,
    ).toBe(value.collectedAt);
  } finally {
    w.unmount();
    controller.dispose();
    vi.useRealTimers();
  }
});
it("filters the directory, restores search and focus, and keeps unknown system state explicit", async () => {
  const w = mount(SystemSettings, { attachTo: document.body });
  const navigation = w.emitted("navigation")![0][0] as PageNavigation;
  try {
    await w.get("input").setValue("网络");
    expect(w.findAll(".settings-overview-grid button")).toHaveLength(1);
    await w.get('[data-setting="network"]').trigger("click");
    expect(document.activeElement).toBe(w.get("h2").element);
    expect(w.text()).toContain("尚未读取系统状态");
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
    expect(await navigation.openNode!("lock")).toBe(true);
    expect(navigation.path.map((e) => e.id)).toEqual(["power", "lock"]);
    expect(await navigation.openNode!("unknown")).toBe(false);
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
