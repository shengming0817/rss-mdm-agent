import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
import Connections from "./Connections.vue";
import { createAssistant } from "./controller";
import { enterCredential, discardCredential } from "../test-users";
vi.mock("../test-users", () => ({
  nativeTestMode: true,
  userGeneration: () => "generation",
  enterCredential: vi.fn(),
  discardCredential: vi.fn().mockResolvedValue(undefined),
}));
it("cancelling replacement of a staged credential clears the deleted reference", async () => {
  vi.mocked(enterCredential)
    .mockResolvedValueOnce("staged-A")
    .mockRejectedValueOnce(new Error("cancelled"));
  const client = {
    initialize: async () => ({}),
    connections: async () => ({
      connections: [],
      preferences: { schemaVersion: 5, kind: "userPreferences" },
    }),
    listSessions: async () => ({ items: [] }),
    observe: () => () => {},
    close: vi.fn(),
    connection: { closed: new Promise(() => {}) },
  } as unknown as RuntimeClient;
  const c = createAssistant(
    {
      connect: async () => ({ runtime: client, mode: "s1" }),
    },
    () => "fixture-id",
  );
  await c.connect();
  const wrapper = mount(Connections, { props: { controller: c } });
  try {
    await flushPromises();
    await wrapper
      .findAll("label")
      .find((l) => l.text().startsWith("认证来源"))!
      .get("select")
      .setValue("custom_api");
    const button = (label: string) =>
      wrapper.findAll("button").find((b) => b.text() === label)!;
    await button("填写安全凭据").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("已选择凭据");
    await button("更换安全凭据").trigger("click");
    await flushPromises();
    expect(discardCredential).toHaveBeenCalledWith("staged-A", "generation");
    expect(wrapper.text()).not.toContain("已选择凭据");
    expect(button("验证并保存").attributes("disabled")).toBeDefined();
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});
