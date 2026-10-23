import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { createLoginFixture } from "../../../../tests/assistant/login-fixture";
import Account from "../settings/Account.vue";
import { UiProvider } from "@rss-mdm-agent/ui";
import { defineComponent, h } from "vue";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
const identity: UserContext = {
  schemaVersion: 1,
  kind: "userContext",
  generation: "fixture-login",
  user: {
    schemaVersion: 1,
    kind: "testUser",
    userId: "fixture",
    nameKey: "fixture",
    displayName: "Fixture",
  },
  identity: {
    mode: "device",
    tenantId: "fixture",
    principalId: "fixture",
    authorityId: "desktop-test",
  },
};
function setup() {
  const fixture = createLoginFixture(async () => identity);
  const wrapper = mount(
    defineComponent({
      setup: () => () =>
        h(UiProvider, { platform: "macos" }, () =>
          h(Account, { loading: false, fixture }),
        ),
    }),
    { attachTo: document.body },
  );
  return { fixture, wrapper, account: wrapper.getComponent(Account) };
}
it("submits account and password together and clears the password immediately", async () => {
  const { wrapper, account } = setup();
  await flushPromises();
  await account.get('[aria-label="企业账号"]').setValue("lin");
  await account.get('[aria-label="企业账号密码"]').setValue("secret");
  await account.get("form").trigger("submit");
  expect(account.emitted("login")).toEqual([
    [{ organizationId: "qinghe", login: "lin", password: "secret" }],
  ]);
  expect(
    (account.get('[aria-label="企业账号密码"]').element as HTMLInputElement)
      .value,
  ).toBe("");
  wrapper.unmount();
});
it("rejects unsigned-in or foreign-organization clients and cancels delayed authorization on disposal", async () => {
  vi.useFakeTimers();
  const { fixture, wrapper } = setup();
  await flushPromises();
  fixture.scenario("mismatch");
  await vi.advanceTimersByTimeAsync(250);
  await fixture.loginClient("lark");
  expect(fixture.state.current).toBeUndefined();
  fixture.scenario("normal");
  await vi.advanceTimersByTimeAsync(250);
  const request = fixture.loginClient("zt").catch((error) => error);
  wrapper.unmount();
  await vi.advanceTimersByTimeAsync(250);
  expect((await request).message).toContain("cancelled");
  expect(fixture.state.current).toBeUndefined();
  vi.useRealTimers();
});
it("requires scanning before phone approval and rejects expired QR confirmations", async () => {
  vi.useFakeTimers();
  const { fixture, wrapper } = setup();
  await flushPromises();
  fixture.refreshQr();
  await fixture.confirm();
  expect(fixture.state.current).toBeUndefined();
  fixture.scan();
  const pending = fixture.confirm();
  fixture.expire();
  await vi.advanceTimersByTimeAsync(250);
  await pending;
  expect(fixture.state.current).toBeUndefined();
  wrapper.unmount();
  vi.useRealTimers();
});
