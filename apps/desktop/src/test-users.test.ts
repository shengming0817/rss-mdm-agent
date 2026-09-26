import { beforeEach, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.vue";
import {
  currentUser,
  accountNotice,
  refreshAccount,
  accountErrorMessage,
  loadOrganizations,
  type Organization,
} from "./test-users";
import Account from "./settings/Account.vue";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
const settingsPage = (
  organizations: Organization[] = [],
  selected?: string,
) => ({
  schemaVersion: 5,
  kind: "accountSettings",
  organizations,
  ...(selected ? { selected } : {}),
});
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
  accountNotice.value = "";
  vi.mocked(invoke).mockReset();
});
it("keeps settings selected when navigation needs an unselected user", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 5, kind: "testUserPage", users: [] };
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  try {
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "AI 助手")!
      .trigger("click");
    expect(wrapper.get('[aria-current="page"]').text()).toBe("设置");
    expect(wrapper.get("h1").text()).toBe("设置");
  } finally {
    wrapper.unmount();
  }
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
    const wrapper = mount(App, {
      global: {
        stubs: {
          Workspace: {
            name: "Workspace",
            template: '<div><slot name="user" /></div>',
          },
        },
      },
    });
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

it("keeps the current workspace mounted and inert while a switch is pending", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return {
        schemaVersion: 5,
        kind: "testUserPage",
        users: [current.user],
        current,
      };
    return new Promise((resolve) => (finish = resolve));
  });
  const wrapper = mount(App, {
    global: {
      stubs: {
        Workspace: {
          name: "Workspace",
          template: '<div><slot name="user" /></div>',
        },
      },
    },
  });
  await flushPromises();
  const workspace = wrapper.findComponent({ name: "Workspace" }).element;
  await wrapper.get('[aria-label="测试用户名"]').setValue("Bob");
  await wrapper.get("form").trigger("submit");
  await flushPromises();
  expect(wrapper.findComponent({ name: "Workspace" }).element).toBe(workspace);
  expect(
    wrapper
      .findComponent({ name: "Workspace" })
      .element.parentElement?.hasAttribute("inert"),
  ).toBe(true);
  finish(current);
  await flushPromises();
  wrapper.unmount();
});

it("keeps enterprise, test and guest entries visible before selecting a user", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 5, kind: "testUserPage", users: [] };
    if (command === "account_organizations") return settingsPage();
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("企业登录");
    expect(wrapper.find('[aria-label="测试用户名"]').exists()).toBe(true);
    const guest = wrapper
      .findAll("button")
      .find((b) => b.text() === "不登录使用");
    expect(guest).toBeDefined();
    await guest!.trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("select_guest");
  } finally {
    wrapper.unmount();
  }
});

it("selects the normalized saved organization and emits the selected server login", async () => {
  const normalized = {
    id: "org",
    label: "Example",
    origin: "https://mdm.example.com",
    tenantId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  };
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "account_organizations"
      ? settingsPage()
      : settingsPage([normalized], normalized.id),
  );
  const wrapper = mount(Account, { props: { loading: false } });
  await flushPromises();
  await wrapper.get('[aria-label="组织名称"]').setValue(" Example ");
  await wrapper
    .get('[aria-label="组织服务地址"]')
    .setValue("https://mdm.example.com/");
  await wrapper
    .get('[aria-label="租户 UUID"]')
    .setValue(normalized.tenantId.toUpperCase());
  await wrapper.findAll("form")[1]!.trigger("submit");
  await flushPromises();
  await wrapper.get('[aria-label="企业账号"]').setValue("Alice");
  await wrapper.findAll("form")[0]!.trigger("submit");
  expect(wrapper.emitted("login")).toEqual([["org", "Alice"]]);
  expect(wrapper.find('input[type="password"]').exists()).toBe(false);
  wrapper.unmount();
});

it("explains enterprise execution limits without invoking a fixture snapshot and reports revoked access", async () => {
  const enterprise = {
    ...current,
    identity: {
      mode: "enterprise" as const,
      authorityId: "mdm-instance",
      tenantId: "tenant",
      principalId: "subject",
      organizationId: "org",
      expiresAtMs: Date.now() + 100000,
    },
  };
  let revoked = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return {
        schemaVersion: 5,
        kind: "testUserPage",
        users: [],
        current: enterprise,
      };
    if (command === "account_organizations") return settingsPage();
    if (command === "account_status")
      return {
        schemaVersion: 5,
        kind: "accountStatus",
        ...(revoked ? {} : { current: enterprise }),
      };
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  await flushPromises();
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "软件中心")!
    .trigger("click");
  await flushPromises();
  expect(wrapper.text()).toContain("企业设备执行与批准尚未接线");
  expect(
    vi.mocked(invoke).mock.calls.some(([c]) => c === "self_service_snapshot"),
  ).toBe(false);
  revoked = true;
  await refreshAccount();
  await flushPromises();
  expect(currentUser.value).toBeUndefined();
  expect(
    wrapper.find('[aria-label="企业账户"] [role="alert"]').text(),
  ).toContain("重新登录");
  wrapper.unmount();
});

it("keeps visible labels for every account input", async () => {
  vi.mocked(invoke).mockResolvedValue(settingsPage());
  const wrapper = mount(Account, { props: { loading: false } });
  await flushPromises();
  for (const name of [
    "组织连接",
    "企业账号",
    "组织名称",
    "组织服务地址",
    "租户 UUID",
  ]) {
    const input = wrapper.get(`[aria-label="${name}"]`);
    const id = input.attributes("id");
    expect(id).toBeTruthy();
    expect(wrapper.get(`label[for="${id}"]`).text()).toContain(name);
  }
  wrapper.unmount();
});
it("returns keyboard focus to settings after the selected workspace is replaced", async () => {
  let active: typeof current | undefined;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return {
        schemaVersion: 5,
        kind: "testUserPage",
        users: active ? [active.user] : [],
        current: active,
      };
    if (command === "select_test_user") {
      active = current;
      return active;
    }
    if (command === "account_organizations") return settingsPage();
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App, { attachTo: document.body });
  try {
    await flushPromises();
    await wrapper.get('[aria-label="测试用户名"]').setValue("Alice");
    (
      wrapper.get('[aria-label="测试用户名"]').element as HTMLInputElement
    ).focus();
    await wrapper.findAll("form")[0]!.trigger("submit");
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get(".settings h1").element);
  } finally {
    wrapper.unmount();
  }
});

it("projects closed account errors without exposing native details and rejects invalid IPC settings", async () => {
  expect(
    accountErrorMessage({
      code: "account/login/unavailable",
      message: "SECRET",
    }),
  ).toContain("网络或服务");
  expect(accountErrorMessage({ code: "account/login/rate_limited" })).toContain(
    "限流",
  );
  expect(
    accountErrorMessage({ code: "account/authorization/denied" }),
  ).toContain("业务授权");
  expect(
    accountErrorMessage({ code: "account/configuration/configuration" }),
  ).toContain("租户 UUID");
  expect(
    accountErrorMessage({ code: "unknown", message: "SECRET" }),
  ).not.toContain("SECRET");
  vi.mocked(invoke).mockResolvedValue({
    ...settingsPage(),
    organizations: [
      {
        id: "x",
        label: "x",
        origin: "https://example.com",
        tenantId: "tenant",
        password: "SECRET",
      },
    ],
  });
  await expect(loadOrganizations()).rejects.toThrow();
});
