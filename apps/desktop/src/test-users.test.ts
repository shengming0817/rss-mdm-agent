import { beforeEach, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.vue";
import {
  currentUser,
  accountNotice,
  accountBinding,
  refreshAccount,
  logoutAccount,
  accountErrorMessage,
  loadAccountSettings,
  type Organization,
} from "./test-users";
import Account from "./settings/Account.vue";
import Workspace from "./Workspace.vue";
import { createHostSettings } from "./settings/controller";

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: vi.fn(),
}));
const settingsPage = (
  organizations: Organization[] = [],
  selected?: string,
) => ({
  schemaVersion: 1,
  kind: "accountSettings",
  organizations,
  ...(selected ? { selected } : {}),
});
const current = {
  schemaVersion: 1 as const,
  kind: "userContext" as const,
  user: {
    schemaVersion: 1 as const,
    kind: "testUser" as const,
    userId: "a",
    displayName: "Alice",
    nameKey: "alice",
  },
  generation: "generation-a",
  identity: {
    mode: "device" as const,
    authorityId: "desktop-test",
    tenantId: "local-test",
    principalId: "a",
  },
};
beforeEach(() => {
  currentUser.value = undefined;
  accountNotice.value = "";
  accountBinding.value = "unbound";
  vi.mocked(invoke).mockReset();
});
it("installs the native device workspace after logout retires the old generation", async () => {
  currentUser.value = { ...current, generation: "old-enterprise-generation" };
  const device = { ...current, generation: "new-device-generation" };
  vi.mocked(invoke).mockImplementation(async (command) => {
    expect(currentUser.value).toBeUndefined();
    if (command === "account_logout") return;
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        current: device,
      };
    throw Error("unexpected native request");
  });
  await logoutAccount();
  expect(currentUser.value).toEqual(device);
  expect(currentUser.value?.identity.mode).toBe("device");
});
it("keeps the retired identity unavailable when logout cleanup is unconfirmed", async () => {
  currentUser.value = current;
  vi.mocked(invoke).mockRejectedValue({ code: "logout_unconfirmed" });
  await expect(logoutAccount()).rejects.toEqual({ code: "logout_unconfirmed" });
  expect(currentUser.value).toBeUndefined();
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
    "account_logout",
    "account_status",
  ]);
});
it("closes assistant modals when leaving the page and preserves the conversation draft", async () => {
  currentUser.value = current;
  vi.mocked(invoke).mockRejectedValue({ code: "ai_unavailable" });
  const wrapper = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "assistant",
      host: createHostSettings(undefined),
    },
  });
  try {
    await flushPromises();
    await wrapper.get(".composer textarea").setValue("继续核对设备状态");
    await wrapper.get('[aria-label="打开主导航"]').trigger("click");
    expect(!!document.querySelector('[role="dialog"][aria-modal="true"]')).toBe(
      true,
    );
    await wrapper.setProps({ page: "tasks" });
    await flushPromises();
    expect(!!document.querySelector('[role="dialog"][aria-modal="true"]')).toBe(
      false,
    );
    await wrapper.setProps({ page: "assistant" });
    expect(!!document.querySelector('[role="dialog"][aria-modal="true"]')).toBe(
      false,
    );
    expect(
      (wrapper.get(".composer textarea").element as HTMLTextAreaElement).value,
    ).toBe("继续核对设备状态");
  } finally {
    wrapper.unmount();
  }
});
it("keeps device pages navigable without an enterprise account", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 1, kind: "testUserPage", users: [] };
    if (command === "select_device") return current;
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  try {
    await flushPromises();
    expect(wrapper.find(".login-screen").exists()).toBe(false);
    const assistant = wrapper
      .findAll("button")
      .find((b) => b.text() === "AI 助手");
    if (assistant) await assistant.trigger("click");
    await flushPromises();
    expect(wrapper.findComponent(Workspace).props("ready")).toBe(true);
    if (assistant)
      expect(wrapper.findComponent(Workspace).props("page")).toBe("assistant");
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
          schemaVersion: 1,
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
      await wrapper.get(".test-users form").trigger("submit");
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
        schemaVersion: 1,
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
  await wrapper.get(".test-users form").trigger("submit");
  await flushPromises();
  expect(wrapper.findComponent({ name: "Workspace" }).element).toBe(workspace);
  expect(
    wrapper.findComponent({ name: "Workspace" }).element.hasAttribute("inert"),
  ).toBe(true);
  finish(current);
  await flushPromises();
  wrapper.unmount();
});

it("opens the device workspace without enterprise login", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return { schemaVersion: 1, kind: "testUserPage", users: [] };
    if (command === "account_organizations") return settingsPage();
    if (command === "select_device") return current;
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  try {
    await flushPromises();
    expect(wrapper.find(".login-screen").exists()).toBe(false);
    expect(wrapper.findComponent(Workspace).props("ready")).toBe(true);
    expect(currentUser.value?.identity.mode).toBe("device");
    expect(invoke).toHaveBeenCalledWith("select_device");
  } finally {
    wrapper.unmount();
  }
});

it.each([
  "binding_unknown",
  "cleanup_pending",
  "conflict",
  "invalidated",
] as const)(
  "allows editing and submitting another login after %s",
  async (binding) => {
    accountBinding.value = binding;
    const organization = {
      id: "enterprise",
      label: "Enterprise",
      origin: "https://mdm.fixture.test",
      tenantId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    };
    vi.mocked(invoke).mockResolvedValue(
      settingsPage([organization], organization.id),
    );
    const wrapper = mount(Account, { props: { loading: false } });
    try {
      await flushPromises();
      const login = wrapper.get('[aria-label="企业账号"]');
      const password = wrapper.get('[aria-label="企业账号密码"]');
      expect(login.attributes("disabled")).toBeUndefined();
      expect(password.attributes("disabled")).toBeUndefined();
      await login.setValue("another-account");
      await password.setValue("another-password");
      await wrapper.get("form").trigger("submit");
      expect(wrapper.emitted("login")?.[0]).toEqual([
        {
          organizationId: organization.id,
          login: "another-account",
          password: "another-password",
        },
      ]);
    } finally {
      wrapper.unmount();
    }
  },
);

it("preselects the embedded organization without logging in and allows another organization", async () => {
  const embedded = {
    id: "embedded",
    label: "Default",
    origin: "https://mdm.fixture.test",
    tenantId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  };
  const custom = {
    ...embedded,
    id: "custom",
    origin: "https://other.fixture.test",
  };
  vi.mocked(invoke).mockResolvedValue(
    settingsPage([embedded, custom], embedded.id),
  );
  const wrapper = mount(Account, {
    props: { loading: false },
    attachTo: document.body,
  });
  await flushPromises();
  expect(wrapper.get(".organization-card").text()).toContain("Default");
  expect(wrapper.emitted("login")).toBeUndefined();
  await wrapper.get(".organization-card").trigger("click");
  await flushPromises();
  const dialog = new DOMWrapper(document.body);
  await dialog.findAll(".organization-row")[1]!.trigger("click");
  await wrapper.get('[aria-label="企业账号"]').setValue("Alice");
  await wrapper.get('[aria-label="企业账号密码"]').setValue("secret");
  await wrapper.get("form").trigger("submit");
  expect(wrapper.emitted("login")).toEqual([
    [{ organizationId: "custom", login: "Alice", password: "secret" }],
  ]);
  wrapper.unmount();
});

it("selects the normalized saved organization and submits complete credentials", async () => {
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
  const wrapper = mount(Account, {
    props: { loading: false },
    attachTo: document.body,
  });
  await flushPromises();
  await wrapper.get(".organization-card").trigger("click");
  await flushPromises();
  const dialog = new DOMWrapper(document.body);
  await dialog.get(".add-organization").trigger("click");
  await dialog.get('[aria-label="组织名称"]').setValue(" Example ");
  await dialog
    .get('[aria-label="组织服务地址"]')
    .setValue("https://mdm.example.com/");
  await dialog
    .get('[aria-label="租户 UUID"]')
    .setValue(normalized.tenantId.toUpperCase());
  await dialog.get(".organization-form").trigger("submit");
  await flushPromises();
  await wrapper.get('[aria-label="企业账号"]').setValue("Alice");
  await wrapper.get('[aria-label="企业账号密码"]').setValue("secret");
  await wrapper.get("form").trigger("submit");
  expect(wrapper.emitted("login")).toEqual([
    [{ organizationId: "org", login: "Alice", password: "secret" }],
  ]);
  expect(
    (wrapper.get('input[type="password"]').element as HTMLInputElement).value,
  ).toBe("");
  wrapper.unmount();
});

it("queries the production execution service without a fixture fallback and reports revoked access", async () => {
  const enterprise = {
    ...current,
    identity: {
      mode: "user_device" as const,
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
        schemaVersion: 1,
        kind: "testUserPage",
        users: [],
        current: enterprise,
      };
    if (command === "account_organizations") return settingsPage();
    if (command === "account_status")
      return {
        schemaVersion: 1,
        kind: "accountStatus",
        binding: "unbound",
        ...(revoked ? {} : { current: enterprise }),
      };
    throw { code: "ai_unavailable" };
  });
  const wrapper = mount(App);
  await flushPromises();
  const nav = wrapper.find('[aria-label="打开主导航"]');
  if (nav.exists()) await nav.trigger("click");
  wrapper.getComponent(Workspace).vm.$emit("navigate", "software");
  await flushPromises();
  expect(wrapper.text()).toContain("无法读取执行服务；原任务保留");
  expect(vi.mocked(invoke).mock.calls.some(([c]) => c === "task_list")).toBe(
    true,
  );
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
  for (const name of ["企业账号", "企业账号密码"]) {
    const input = wrapper.get(`[aria-label="${name}"]`);
    expect(
      wrapper.get(`label[for="${input.attributes("id")}"]`).text(),
    ).toBeTruthy();
  }
  wrapper.unmount();
});
it("focuses the default AI page after the selected workspace is replaced", async () => {
  let active: typeof current | undefined;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "test_users")
      return {
        schemaVersion: 1,
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
    wrapper.getComponent(Workspace).vm.$emit("navigate", "settings");
    await flushPromises();
    await wrapper.get('[aria-label="测试用户名"]').setValue("Alice");
    (
      wrapper.get('[aria-label="测试用户名"]').element as HTMLInputElement
    ).focus();
    await wrapper.findAll("form")[0]!.trigger("submit");
    await flushPromises();
    expect(document.activeElement).toBe(
      wrapper.get(".shell-header h1").element,
    );
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
  await expect(loadAccountSettings()).rejects.toThrow();
});

it.each(["cleanup_pending", "conflict"] as const)(
  "exposes recovery after a rejected logout with %s",
  async (binding) => {
    currentUser.value = current;
    accountBinding.value = "bound";
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "account_logout") throw { code: "cleanup_pending" };
      if (command === "account_status")
        return { schemaVersion: 1, kind: "accountStatus", binding };
      throw Error("unexpected request");
    });
    await expect(logoutAccount()).rejects.toEqual({ code: "cleanup_pending" });
    expect(currentUser.value).toBeUndefined();
    expect(accountBinding.value).toBe(binding);
    const wrapper = mount(Account, { props: { loading: false } });
    await flushPromises();
    expect(wrapper.text()).toContain("重试原操作");
    expect(wrapper.text()).toContain("清理后重新登录");
    expect(
      wrapper.get('[aria-label="企业账号密码"]').attributes("disabled"),
    ).toBeUndefined();
    wrapper.unmount();
  },
);
