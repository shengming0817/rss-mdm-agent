import { mount } from "@vue/test-utils";
import { expect, it } from "vitest";
import type { PageNavigation } from "../workspace/page-navigation";
import SoftwareManagement from "./SoftwareManagement.vue";
import { softwarePreviewKey } from "./software-preview";
import { createSoftwarePreview } from "../../../../tests/assistant/software-preview";

it("uses only injected preview data and preserves the installed/search view on detail return", async () => {
  const empty = mount(SoftwareManagement);
  expect(empty.findAll(".software-item")).toHaveLength(0);
  empty.unmount();
  const model = createSoftwarePreview();
  const wrapper = mount(SoftwareManagement, {
    attachTo: document.body,
    global: { provide: { [softwarePreviewKey]: model } },
  });
  const navigation = wrapper.emitted("navigation")![0][0] as PageNavigation;
  navigation.openSection("installed");
  await wrapper.vm.$nextTick();
  expect(wrapper.findAll(".software-item")).toHaveLength(4);
  await wrapper.get("input").setValue("Notes");
  await wrapper.get('[data-software="notes"]').trigger("click");
  expect(wrapper.get(".software-detail-hero").text()).toContain("版本 2.1");
  await wrapper.get(".software-detail-back").trigger("click");
  expect(wrapper.get("input").element.value).toBe("Notes");
  expect(document.activeElement).toBe(
    wrapper.get('[data-software="notes"]').element,
  );
  wrapper.unmount();
});

it("limits simulated writes to the current sample revision and refuses managed removal", () => {
  const model = createSoftwarePreview();
  expect(model.apply("sync", "uninstall", 0)).toBe(false);
  expect(model.apply("pdf", "install", 0)).toBe(true);
  expect(model.items.find((item) => item.id === "pdf")?.version).toBe("5.1");
  expect(model.apply("workspace", "update", 0)).toBe(false);
  expect(model.apply("workspace", "update", 1)).toBe(true);
  expect(model.apply("pdf", "uninstall", 2)).toBe(true);
  expect(model.items.find((item) => item.id === "pdf")?.installed).toBe(false);
});
