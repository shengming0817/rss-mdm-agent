import { mount, flushPromises } from "@vue/test-utils";
import { expect, it } from "vitest";
import ToolsPage from "./ToolsPage.vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
it("uses the script-only shared directory and routes diagnostics and system settings to real destinations", async () => {
  const w = mount(ToolsPage);
  try {
    await flushPromises();
    expect(w.getComponent(PolicySelfService).props("kind")).toBe("script");
    expect(w.text()).not.toContain("尚未连接实际检查");
    await w.get("[data-tool=diagnostics]").trigger("click");
    await w
      .findAll("button")
      .find((b) => b.text().includes("预览并导出"))!
      .trigger("click");
    expect(w.emitted("navigate")!.at(-1)).toEqual(["settings", "diagnostics"]);
    await w
      .findAll("button")
      .find((b) => b.text().includes("返回工具目录"))!
      .trigger("click");
    await w.get("[data-tool=power]").trigger("click");
    await w
      .findAll("button")
      .find((b) => b.text().includes("打开对应系统设置"))!
      .trigger("click");
    expect(w.emitted("navigate")!.at(-1)).toEqual(["system-settings", "power"]);
  } finally {
    w.unmount();
  }
});
