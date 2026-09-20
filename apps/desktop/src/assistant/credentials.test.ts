import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
import Connections from "./Connections.vue";
import { createAssistant } from "./controller";
import { saveNativeConnection } from "../test-users";
vi.mock("../test-users", () => ({
  nativeTestMode: true,
  saveNativeConnection: vi.fn(),
}));
it("custom input is acquired only by native save; cancellation leaves the form retryable without staged credentials", async () => {
  vi.mocked(saveNativeConnection).mockRejectedValueOnce({ code: "cancelled" });
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
    { connect: async () => ({ runtime: client, mode: "s1" }) },
    () => "fixture-id",
  );
  await c.connect();
  const wrapper = mount(Connections, { props: { controller: c } });
  try {
    await flushPromises();
    const label = (name: string) =>
      wrapper.findAll("label").find((l) => l.text().startsWith(name))!;
    await label("认证来源").get("select").setValue("custom_api");
    await label("名称").get("input").setValue("Custom");
    await label("API 地址").get("input").setValue("https://example.invalid");
    await label("模型").get("input").setValue("chosen");
    expect(saveNativeConnection).not.toHaveBeenCalled();
    expect(wrapper.findAll('input[type="password"]')).toHaveLength(0);
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    const [metadata, expected, replace] =
      vi.mocked(saveNativeConnection).mock.calls[0]!;
    expect(metadata.source.type).toBe("custom_api");
    expect(expected).toBeNull();
    expect(replace).toBe(true);
    expect(Object.keys(metadata)).not.toEqual(
      expect.arrayContaining(["credentialRef", "accountRef"]),
    );
    expect(label("名称").get("input").element.value).toBe("Custom");
    expect(
      wrapper
        .findAll("button")
        .find((b) => b.text() === "验证并保存")!
        .attributes("disabled"),
    ).toBeUndefined();
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});
