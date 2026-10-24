import { mount, flushPromises } from "@vue/test-utils";
import { expect, it } from "vitest";
import SoftwareManagement from "./SoftwareManagement.vue";
import { createSoftware } from "./controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import { currentUser } from "../test-users";
import type { PageNavigation } from "../workspace/page-navigation";
it("uses the production model, retains its filtered list and ignores same-generation account refresh", async () => {
  const ports = fixtureLocalPorts(),
    model = createSoftware(ports.data, ports.software);
  const w = mount(SoftwareManagement, {
    attachTo: document.body,
    props: { model, active: true },
  });
  const old = currentUser.value;
  try {
    await flushPromises();
    const nav = w.emitted("navigation")![0][0] as PageNavigation;
    expect(nav.searchEntries).toHaveLength(4);
    await w.get('input[aria-label="搜索软件"]').setValue("Notes");
    await w.get('[data-software="notes"]').trigger("click");
    expect(w.get('[aria-label="软件详情"]').text()).toContain("Notes");
    await w.get(".software-detail-back").trigger("click");
    expect(
      (w.get('input[aria-label="搜索软件"]').element as HTMLInputElement).value,
    ).toBe("Notes");
    expect(document.activeElement).toBe(
      w.get('[data-software="notes"]').element,
    );
    currentUser.value = {
      kind: "userContext",
      schemaVersion: 1,
      generation: "refresh",
      user: {
        kind: "testUser",
        schemaVersion: 1,
        userId: "u",
        displayName: "U",
        nameKey: "u",
      },
      identity: {
        mode: "device",
        authorityId: "a",
        tenantId: "t",
        principalId: "u",
      },
    };
    await flushPromises();
    currentUser.value = structuredClone(currentUser.value);
    await flushPromises();
    await model.discover();
    expect(model.installed.value).toHaveLength(4);
    expect(await nav.openResource!("notes", "wrong-revision")).toBe(false);
  } finally {
    w.unmount();
    model.dispose();
    currentUser.value = old;
  }
});
