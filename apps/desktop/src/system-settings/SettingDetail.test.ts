import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import SettingDetail from "./SettingDetail.vue";
import { createSystemSettings, systemSettingsKey } from "./controller";
import type {
  SettingsOperation,
  SettingsResponse,
} from "../self-service/types";

it("submits only writable proxy edits and offers draft adoption after unrelated refresh", async () => {
  const calls: SettingsOperation[] = [];
  const response: SettingsResponse = {
    writeAllowed: true,
    request: null,
    digest: null,
    finished: true,
    effect: "observed",
    keepBeforeUnixMs: null,
    reason: null,
    snapshot: {
      section: "network",
      revision: "a".repeat(64),
      collectedAtUnixMs: 1,
      source: "native fixture",
      targets: [
        {
          id: "proxy",
          name: "Proxy",
          options: [],
          reason: null,
          fields: [
            ...["http", "https", "socks"].map((key) => ({
              key,
              value: null,
              writable: false,
              quality: "unknown" as const,
              reason: "unparsed endpoint",
            })),
            {
              key: "bypass",
              value: [],
              writable: true,
              quality: "complete",
              reason: null,
            },
          ],
        },
      ],
    },
  };
  const controller = createSystemSettings(async (operation) => {
    calls.push(operation);
    return structuredClone(response);
  }, null);
  const wrapper = mount(SettingDetail, {
    props: { section: "network" },
    global: {
      provide: { [systemSettingsKey as symbol]: controller },
      stubs: {
        UiDialog: {
          props: ["open"],
          template: '<div v-if="open"><slot /></div>',
        },
      },
    },
  });
  try {
    await controller.load("network");
    await nextTick();
    expect(
      wrapper
        .findAll("input")
        .every((input) => input.attributes("disabled") !== undefined),
    ).toBe(true);
    await wrapper.get("textarea").setValue("intranet.example");
    response.snapshot!.revision = "b".repeat(64);
    response.snapshot!.targets[0].name = "Renamed service";
    await controller.load("network");
    await nextTick();
    expect(wrapper.text()).toContain("放弃草稿，采用最新状态");
    await wrapper.get("form").trigger("submit");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "确认修改")!
      .trigger("click");
    expect(
      calls.find((operation) => operation.kind === "change"),
    ).toMatchObject({
      kind: "change",
      expected: "a".repeat(64),
      patch: {
        kind: "proxy",
        target: "proxy",
        http: null,
        https: null,
        socks: null,
        bypass: ["intranet.example"],
      },
    });
  } finally {
    wrapper.unmount();
    controller.dispose();
  }
});
