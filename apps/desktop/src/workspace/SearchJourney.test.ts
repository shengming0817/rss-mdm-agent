import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import Workspace from "../Workspace.vue";
import WorkspaceSearch from "./WorkspaceSearch.vue";
import ToolsPage from "../device-tools/ToolsPage.vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import { createHostSettings } from "../settings/controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import {
  policyFixturePort,
  directoryItem,
  policyRecord,
  taskPage,
} from "../../../../tests/self-service/support";
import type {
  SelfServiceDirectoryItem,
  TaskQuery,
  TaskRef,
} from "../self-service/types";

it("searches subsequent authorized pages through the same directory and opens the exact policy without executing", async () => {
  const item: SelfServiceDirectoryItem = {
    capability: directoryItem(policyRecord().action).capability,
    policyId: "policy-17",
    policyRevision: 1,
    policyVersion: "version-1",
    subject: { kind: "device" },
    task: {
      kind: "script",
      runAs: "system",
      resource: { id: "resource-17", version: "1", variant: "macos" },
      parameterSchema: { type: "object" },
      parameterSources: {},
    },
    selfService: {
      published: true,
      allowAi: true,
      riskLevel: 1,
      presentation: {
        displayName: "Late authorized tool",
        description: "Read-only tool",
        category: "Tools",
        keywords: [],
        prerequisites: "Current user",
        sideEffects: "Read-only",
      },
    },
  };
  const directory = vi.fn(async ({ after }) => {
    const page = Number(after ?? 0);
    return {
      items: page === 17 ? [item] : [],
      nextCursor: page < 17 ? String(page + 1) : null,
      asOf: 1000,
    };
  });
  const invoke = vi.fn(),
    local = fixtureLocalPorts();
  const submit = vi.spyOn(local.data!, "query_submit");
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      page: "home",
      fixture: true,
      localPorts: local,
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        policy_directory: directory,
        policy_invoke: invoke,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
    },
  });
  try {
    await flushPromises();
    const search = w.getComponent(WorkspaceSearch);
    expect(directory).not.toHaveBeenCalled();
    await w.get('button[aria-label="搜索应用、工具与设置"]').trigger("click");
    await flushPromises();
    expect(search.props("complete")).toBe(false);
    for (let page = 0; page < 17; page++) {
      search.vm.$emit("more");
      await flushPromises();
    }
    const entry = search
      .props("entries")
      .find((entry) => entry.id === "policy:policy-17")!;
    expect(entry).toMatchObject({
      kind: "resource",
      page: "device-tools",
      title: "Late authorized tool",
    });
    search.vm.$emit("select", entry);
    await w.setProps({ page: "device-tools" });
    await flushPromises();
    expect(
      w
        .getComponent(ToolsPage)
        .getComponent(PolicySelfService)
        .find("form")
        .exists(),
    ).toBe(true);
    expect(directory).toHaveBeenCalledTimes(18);
    expect(invoke).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    search.vm.$emit("select", { ...entry, revision: "old-version" });
    await flushPromises();
    expect(w.text()).toContain("资源已撤回或版本已变化");
    expect(invoke).not.toHaveBeenCalled();
  } finally {
    w.unmount();
  }
});

it("restores early task search results after the bounded window reaches the end", async () => {
  const { taskPort, taskDetail } = await import(
    "../../../../tests/self-service/support"
  );
  const tasks = {
    ...taskPort(),
    list: vi.fn(async (query: TaskQuery) => {
      const page = Number(query.cursor ?? 0);
      return {
        ...taskPage(
          Array.from({ length: 32 }, (_, i) =>
            taskDetail(`search-${page}-${i}`),
          ),
        ),
        next: page < 16 ? String(page + 1) : null,
      };
    }),
    detail: vi.fn(async (reference: TaskRef) => {
      if (reference.kind !== "request") throw Error("request fixture");
      return taskDetail(reference.request);
    }),
    control: vi.fn(),
  };
  const w = mount(Workspace, {
    attachTo: document.body,
    props: {
      ready: true,
      busy: false,
      fixture: true,
      page: "home",
      host: createHostSettings(undefined),
      selfServicePort: {
        ...policyFixturePort,
        execute: vi.fn(),
        confirm: vi.fn(),
      },
      localPorts: {
        tasks,
        data: null,
        software: null,
        settings: null,
        network: null,
      },
      onNavigate: (page: string) => {
        void w.setProps({ page });
      },
    },
  });
  try {
    await flushPromises();
    const search = w.getComponent(WorkspaceSearch);
    for (let page = 0; page < 16; page++) {
      search.vm.$emit("more");
      await flushPromises();
    }
    expect(tasks.list).toHaveBeenCalledTimes(17);
    expect(search.props("complete")).toBe(false);
    expect(search.props("hasMore")).toBe(false);
    expect(search.props("restartable")).toBe(true);
    search.vm.$emit("restart");
    await flushPromises();
    const entry = search
      .props("entries")
      .find(
        (e) =>
          e.id === JSON.stringify({ kind: "request", request: "search-0-0" }),
      );
    expect(entry).toBeDefined();
    search.vm.$emit("select", entry);
    await flushPromises();
    expect(tasks.detail).toHaveBeenCalledWith({
      kind: "request",
      request: "search-0-0",
    });
    expect(tasks.control).not.toHaveBeenCalled();
  } finally {
    w.unmount();
  }
});
