import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import SettingDetail from "./SettingDetail.vue";
import { createSystemSettings, systemSettingsKey } from "./controller";
import type {
  SettingsOperation,
  SettingsResponse,
  LocalPreparation,
} from "../self-service/types";

it("prepares an exact external patch, rejects changed snapshots, and retains the original confirmation flow", async () => {
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
      section: "power",
      revision: "a".repeat(64),
      collectedAtUnixMs: 1,
      source: "native fixture",
      targets: [
        {
          id: "ac",
          name: "AC",
          options: [],
          reason: null,
          fields: [
            {
              key: "displayOffMinutes",
              value: 5,
              writable: true,
              quality: "complete",
              reason: null,
            },
          ],
        },
      ],
    },
  };
  const controller = createSystemSettings(async (input) => {
    calls.push(input);
    return structuredClone(response);
  }, null);
  const w = mount(SettingDetail, {
    props: { section: "power" },
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
  const input: Extract<LocalPreparation, { kind: "settings" }> = {
    kind: "settings",
    section: "power",
    action: "change",
    expected: "a".repeat(64),
    patch: {
      kind: "power",
      target: "ac",
      displayOffMinutes: 10,
      sleepMinutes: null,
      mode: null,
    },
  };
  try {
    expect(
      await w.vm.prepareEntry({ ...input, expected: "b".repeat(64) }),
    ).toBe(false);
    expect(await w.vm.prepareEntry(input)).toBe(true);
    await nextTick();
    expect(calls.every((v) => v.kind === "snapshot")).toBe(true);
    expect(w.get('[aria-label="外部设置建议"]').text()).toContain(
      '"displayOffMinutes": 10',
    );
    await w
      .findAll("button")
      .find((v) => v.text() === "核对外部修改")!
      .trigger("click");
    expect(calls.every((v) => v.kind === "snapshot")).toBe(true);
    await w
      .findAll("button")
      .find((v) => v.text() === "确认修改")!
      .trigger("click");
    expect(calls.find((v) => v.kind === "change")).toMatchObject({
      kind: "change",
      expected: input.expected,
      patch: input.patch,
    });
    const submitted = calls.find((v) => v.kind === "change")!;
    expect("request" in submitted && submitted.request).toMatch(
      /^[0-9a-f-]{36}$/,
    );
  } finally {
    w.unmount();
    controller.dispose();
  }
});

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
              value:
                key === "http" ? { enabled: false, host: "", port: 0 } : null,
              writable: false,
              quality:
                key === "http" ? ("complete" as const) : ("unknown" as const),
              reason:
                key === "http" ? "该字段由组织策略限制" : "unparsed endpoint",
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
    expect(wrapper.text()).toContain("该字段由组织策略限制");
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
