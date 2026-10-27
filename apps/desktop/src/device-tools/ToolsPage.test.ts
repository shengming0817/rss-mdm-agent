import { directoryItem } from "../../../../tests/self-service/support";
import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import ToolsPage from "./ToolsPage.vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import ConnectionCheck from "../device-tools/ConnectionCheck.vue";
import type { NetworkCheckPort } from "../self-service/native";
import type { NetworkCheckStatus } from "../self-service/types";
import { currentUser } from "../test-users";
it("uses the script-only shared directory and routes diagnostics and system settings to real destinations", async () => {
  const w = mount(ToolsPage);
  try {
    await flushPromises();
    expect(w.getComponent(PolicySelfService).props("kind")).toBe("script");
    expect(w.text()).not.toContain("尚未连接实际检查");
    await w.get("[data-tool=diagnostics]").trigger("click");
    await w
      .findAll("button")
      .find((b) => b.text().includes("预览并导出"))!
      .trigger("click");
    expect(w.emitted("navigate")!.at(-1)).toEqual(["settings", "diagnostics"]);
    await w
      .findAll("button")
      .find((b) => b.text().includes("返回工具目录"))!
      .trigger("click");
    await w.get("[data-tool=power]").trigger("click");
    await w
      .findAll("button")
      .find((b) => b.text().includes("打开对应系统设置"))!
      .trigger("click");
    expect(w.emitted("navigate")!.at(-1)).toEqual(["system-settings", "power"]);
  } finally {
    w.unmount();
  }
});

it("retains the original network request across navigation, pauses hidden polling and clears it for another login", async () => {
  vi.useFakeTimers();
  const previousUser = currentUser.value;
  const call = vi.fn(async (input) => {
    if (input.kind === "defaults")
      return { kind: "defaults", value: { host: "mdm.example", port: 8443 } };
    return {
      kind: "status",
      value: {
        status: {
          operationRequestId: input.request,
          phase: input.kind === "run" ? "running" : "verified",
        },
        result:
          input.kind === "run"
            ? null
            : {
                target: { host: "mdm.example", port: 8443 },
                collectedAtUnixMs: 1,
                dns: { address: null, finding: "reachable", durationMs: 1 },
                tcp: [
                  {
                    address: "192.0.2.1:8443",
                    finding: "refused",
                    durationMs: 2,
                  },
                ],
              },
      } as NetworkCheckStatus,
    };
  });
  const w = mount(ToolsPage, {
    attachTo: document.body,
    props: { networkPort: call as NetworkCheckPort },
  });
  const back = async () => {
    await w
      .findAll("button")
      .find((b) => b.text().includes("返回工具目录"))!
      .trigger("click");
    await flushPromises();
  };
  try {
    await w.get("[data-tool=network]").trigger("click");
    await flushPromises();
    await w.getComponent(ConnectionCheck).get("form").trigger("submit");
    await flushPromises();
    const original = call.mock.calls.find(([v]) => v.kind === "run")![0]
      .request;
    await back();
    expect(w.getComponent(ConnectionCheck).isVisible()).toBe(false);
    const hiddenCalls = call.mock.calls.length;
    await vi.advanceTimersByTimeAsync(3000);
    expect(call).toHaveBeenCalledTimes(hiddenCalls);
    await w.get("[data-tool=power]").trigger("click");
    await w.setProps({ active: false });
    await w.setProps({ active: true });
    await back();
    await w.get("[data-tool=network]").trigger("click");
    await flushPromises();
    expect(call.mock.calls.at(-1)![0]).toEqual({
      kind: "status",
      request: original,
    });
    expect(w.getComponent(ConnectionCheck).text()).toContain("DNS：可达");
    expect(w.getComponent(ConnectionCheck).text()).toContain("连接被拒绝");
    await back();
    await w.get("[data-tool=network]").trigger("click");
    await flushPromises();
    expect(w.getComponent(ConnectionCheck).text()).toContain("192.0.2.1:8443");
    expect(call.mock.calls.filter(([v]) => v.kind === "run")).toHaveLength(1);
    await back();
    currentUser.value = {
      schemaVersion: 1,
      kind: "userContext",
      generation: "another-login",
      user: {
        schemaVersion: 1,
        kind: "testUser",
        userId: "other",
        displayName: "Other",
        nameKey: "other",
      },
      identity: {
        mode: "device",
        authorityId: "test",
        tenantId: "test",
        principalId: "other",
      },
    };
    await flushPromises();
    await w.get("[data-tool=network]").trigger("click");
    await flushPromises();
    expect(
      w
        .getComponent(ConnectionCheck)
        .find('[aria-label="原检查请求"]')
        .exists(),
    ).toBe(false);
    expect(w.getComponent(ConnectionCheck).text()).not.toContain(
      "192.0.2.1:8443",
    );
  } finally {
    w.unmount();
    currentUser.value = previousUser;
    vi.useRealTimers();
  }
});

it("keeps the original manual tool request across hidden-page navigation after a lost receipt", async () => {
  const record = policyRecord();
  if (record.action.kind !== "policy") throw Error("policy");
  const item = directoryItem(record.action);
  const invoke = vi.fn(async (_input: { choice: { request: string } }) => {
    throw Error("reply lost");
  });
  const port = {
    ...policyFixturePort,
    policy_directory: async () => ({
      items: [item],
      nextCursor: null,
      asOf: 1,
    }),
    policy_invoke: invoke,
    offers: async () => [],
    execute: vi.fn(),
    confirm: vi.fn(),
  };
  const execution = createTaskCenter(taskPort(), port);
  const w = mount(ToolsPage, { props: { execution, active: true } });
  try {
    await flushPromises();
    const view = w.getComponent(PolicySelfService);
    await view
      .findAll("button")
      .find((b) => b.text() === "Published tool")!
      .trigger("click");
    await view.get("form").trigger("submit");
    await flushPromises();
    expect(invoke).toHaveBeenCalledTimes(1);
    const request = invoke.mock.calls[0][0].choice.request;
    await w.setProps({ active: false });
    await w.setProps({ active: true });
    await flushPromises();
    expect(w.getComponent(PolicySelfService).element).toBe(view.element);
    expect(view.text()).toContain(request);
    expect(view.find("form").exists()).toBe(false);
    expect(invoke).toHaveBeenCalledTimes(1);
  } finally {
    w.unmount();
    execution.dispose();
  }
});
import {
  policyRecord,
  policyFixturePort,
  taskPort,
} from "../../../../tests/self-service/support";
import { createTaskCenter } from "../task-center/controller";
