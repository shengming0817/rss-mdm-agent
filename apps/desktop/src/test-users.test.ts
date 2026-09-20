import { beforeEach, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.vue";
import { currentUser } from "./test-users";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
const current = {
  schemaVersion: 5 as const,
  kind: "userContext" as const,
  user: {
    schemaVersion: 5 as const,
    kind: "testUser" as const,
    userId: "a",
    displayName: "Alice",
    nameKey: "alice",
  },
  generation: "generation-a",
};
beforeEach(() => {
  currentUser.value = undefined;
  vi.mocked(invoke).mockReset();
});
it.each([
  ["invalid_name", "1–64"],
  ["limit", "选择已有用户"],
  ["users_unavailable", "检查本地存储"],
  ["ai_unavailable", "重新连接 AI"],
  ["unknown", "已登记设备任务仍可查询"],
])(
  "preserves the current user and gives an actionable %s failure without raw details",
  async (code, expected) => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "test_users")
        return {
          schemaVersion: 5,
          kind: "testUserPage",
          users: [current.user],
          current,
        };
      throw { code, message: "SECRET_CANARY" };
    });
    const wrapper = mount(App, { global: { stubs: { Workspace: true } } });
    try {
      await flushPromises();
      await wrapper.get('[aria-label="测试用户名"]').setValue("Bob");
      await wrapper.get("form").trigger("submit");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("select_test_user", { name: "Bob" });
      expect(wrapper.get('[role="alert"]').text()).toContain(expected);
      expect(wrapper.text()).not.toContain("SECRET_CANARY");
      expect(wrapper.text()).toContain("当前用户：Alice");
      expect(currentUser.value).toEqual(current);
      expect(wrapper.findComponent({ name: "Workspace" }).exists()).toBe(true);
    } finally {
      wrapper.unmount();
    }
  },
);
