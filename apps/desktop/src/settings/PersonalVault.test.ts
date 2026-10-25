import { mount, flushPromises } from "@vue/test-utils";
import { describe, it, expect, vi, afterEach } from "vitest";
import PersonalVault from "./PersonalVault.vue";
import type { PersonalVaultPort, PersonalVaultStatus } from "./native";
const baseline = (): PersonalVaultStatus => ({
  initialized: true,
  unlocked: true,
  passwordEnabled: true,
  revision: 3,
  entries: [
    {
      credentialId: [1],
      rpId: "example.com",
      userHandle: [1],
      username: "alice",
      displayName: "Alice",
      backupEligible: true,
      backedUp: false,
    },
    {
      credentialId: [2],
      rpId: "example.com",
      userHandle: [2],
      username: "bob",
      displayName: "Bob",
      backupEligible: true,
      backedUp: false,
    },
  ],
});
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.useRealTimers();
});
function app(port: PersonalVaultPort) {
  const wrapper = mount(PersonalVault, { props: { port } });
  wrappers.push(wrapper);
  return wrapper;
}
function button(wrapper: ReturnType<typeof mount>, text: string) {
  const node = wrapper.findAll("button").find((b) => b.text() === text);
  if (!node) throw Error(`missing button: ${text}`);
  return node;
}
describe("personal Vault", () => {
  it("selects the exact account and confirms local-only deletion", async () => {
    const port = vi.fn<PersonalVaultPort>().mockResolvedValue(baseline());
    const wrapper = app(port);
    await flushPromises();
    await wrapper.findAll('input[type="radio"]')[1]!.setValue();
    await button(wrapper, "删除所选本地条目").trigger("click");
    expect(wrapper.text()).toContain("不会删除网站端公钥或其他副本");
    await button(wrapper, "确认删除").trigger("click");
    await flushPromises();
    expect(port).toHaveBeenCalledWith({
      kind: "delete",
      credential_id: [2],
      revision: 3,
    });
  });
  it("requests native password UI without transporting password values", async () => {
    const port = vi.fn<PersonalVaultPort>().mockResolvedValue(baseline());
    const wrapper = app(port);
    await flushPromises();
    expect(wrapper.find('input[type="password"]').exists()).toBe(false);
    await button(wrapper, "口令解锁").trigger("click");
    await flushPromises();
    expect(port).toHaveBeenCalledWith({ kind: "unlock", method: "password" });
    await wrapper.find("select").setValue("password");
    await button(wrapper, "修改个人口令").trigger("click");
    await flushPromises();
    expect(port).toHaveBeenCalledWith({
      kind: "set_password",
      authorization: "password",
    });
  });
  it("ignores a late unlock response after lock", async () => {
    let resolve!: (status: PersonalVaultStatus) => void;
    const port: PersonalVaultPort = (operation) =>
      operation.kind === "unlock"
        ? new Promise((r) => {
            resolve = r;
          })
        : Promise.resolve(
            operation.kind === "lock"
              ? { ...baseline(), unlocked: false, entries: [] }
              : baseline(),
          );
    const wrapper = app(port);
    await flushPromises();
    await button(wrapper, "口令解锁").trigger("click");
    await button(wrapper, "锁定 / 取消").trigger("click");
    await flushPromises();
    resolve(baseline());
    await flushPromises();
    expect(wrapper.text()).toContain("已锁定");
    expect(wrapper.findAll('input[type="radio"]')).toHaveLength(0);
  });
  it("shows a closed failure and requires an explicit reset phrase", async () => {
    const port = vi.fn<PersonalVaultPort>().mockRejectedValue("corrupt");
    const wrapper = app(port);
    await flushPromises();
    expect(wrapper.find('[role="alert"]').text()).toContain("原文件已保留");
    await button(wrapper, "损坏 / 丢失处理").trigger("click");
    const reset = button(wrapper, "系统确认并建立新 Vault");
    expect(reset.attributes("disabled")).toBeDefined();
    await wrapper
      .find('input[autocomplete="off"]')
      .setValue("DELETE LOCAL VAULT");
    await reset.trigger("click");
    await flushPromises();
    expect(port).toHaveBeenCalledWith({
      kind: "reset",
      confirmation: "DELETE LOCAL VAULT",
    });
  });
  it("clears metadata when native polling observes an OS lock", async () => {
    vi.useFakeTimers();
    const port = vi
      .fn<PersonalVaultPort>()
      .mockResolvedValueOnce(baseline())
      .mockResolvedValue({ ...baseline(), unlocked: false, entries: [] });
    const wrapper = app(port);
    await flushPromises();
    expect(wrapper.text()).toContain("Alice");
    await vi.advanceTimersByTimeAsync(1500);
    await flushPromises();
    expect(wrapper.text()).not.toContain("Alice");
  });
});
