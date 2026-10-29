import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { deepClone } from "valtio/vanilla/utils";
import { UiProvider } from "@rss-mdm-agent/ui";
import type { RuntimeClient } from "@rss-mdm-agent/ai-client";
import { shellFixture } from "../../../../tests/desktop/ui-support";
import { createWorkspace } from "../state/workspace";
import { AssistantPage } from "../assistant/AssistantPage";
import { ConnectionsPage } from "./ConnectionsPage";
const cleanup: (() => void)[] = [];
afterEach(() => cleanup.splice(0).forEach((release) => release()));
async function fixture() {
  const f = shellFixture(),
    connect = f.input.assistantServices!.connect;
  let client!: RuntimeClient,
    catalog!: Awaited<ReturnType<RuntimeClient["connections"]>>;
  let workspace!: ReturnType<typeof createWorkspace>;
  f.input.assistantServices!.connect = async (options, signal) => {
    const opened = await connect(options, signal);
    client = opened.runtime;
    catalog = await client.connections();
    catalog.connections.push({
      ...catalog.connections[0],
      connectionId: "B",
      presentation: { name: "第二连接" },
    });
    const restore = client.restore.bind(client),
      create = client.createSession.bind(client);
    Object.assign(client, {
      connections: async () => structuredClone(catalog),
      createSession: vi.fn(create),
      savePreferences: vi.fn(
        async (patch: Parameters<RuntimeClient["savePreferences"]>[0]) => {
          if (patch.defaultConnectionId && "set" in patch.defaultConnectionId)
            catalog.preferences.defaultConnectionId =
              patch.defaultConnectionId.set;
          if (patch.selectedSessionId && "set" in patch.selectedSessionId)
            catalog.preferences.selectedSessionId = patch.selectedSessionId.set;
          return structuredClone(catalog.preferences);
        },
      ),
      restore: vi.fn(async (id: string) => {
        const current = workspace.assistant.state.views.get(id);
        return current
          ? deepClone(current)
          : { ...(await restore(id)), selectedConnectionId: "cfg" };
      }),
      selectConnection: vi.fn(
        async (id: string, connectionId: string, fresh: boolean) => {
          const current = deepClone(workspace.assistant.state.views.get(id)!);
          return {
            ...current,
            selectedConnectionId: connectionId,
            connectionPending:
              fresh || current.selectedConnectionId !== connectionId,
          };
        },
      ),
      previewHistory: vi.fn(async (id: string, connectionId: string) => ({
        schemaVersion: 1,
        kind: "historyPreview",
        sessionId: id,
        connectionId,
        configRevision: 1,
        throughSequence: workspace.assistant.view()!.cursor,
        commandIds: ["original-completed"],
        messageIds: ["stable-message"],
        text: "稳定原文，不包含工具或秘密",
        contentHash: "b".repeat(64),
      })),
      resume: vi.fn(async (id: string) => ({
        ...deepClone(workspace.assistant.state.views.get(id)!),
        sessionStatus: "active",
        generation: "resumed-generation",
      })),
    });
    return opened;
  };
  workspace = createWorkspace(f.current(), f.input);
  const c = workspace.assistant;
  await c.connect();
  c.create();
  c.draft.set("初始化会话");
  await c.prompt();
  f.submit.mockClear();
  const app = render(
    <UiProvider reducedMotion>
      <ConnectionsPage workspace={workspace} active />
    </UiProvider>,
  );
  cleanup.push(() => {
    app.unmount();
    workspace.dispose();
  });
  return { ...f, c, client, catalog, workspace, app };
}
it("exposes default and per-conversation selection through user settings and preserves the selected draft into the actual composer", async () => {
  const f = await fixture(),
    original = f.c.state.selected;
  await act(async () =>
    fireEvent.click(
      screen.getByRole("button", { name: "将 第二连接 设为新会话默认" }),
    ),
  );
  expect(f.client.savePreferences).toHaveBeenCalledWith({
    defaultConnectionId: { set: "B" },
  });
  expect(f.c.selectedConnectionId()).toBe("cfg");
  expect(f.c.state.selected).toBe(original);
  await act(async () => {
    f.c.create();
    f.c.draft.set("未发送的用户草稿");
  });
  expect(f.c.selectedConnectionId()).toBe("B");
  fireEvent.click(screen.getByRole("button", { name: "会话选项" }));
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "本会话连接" }));
  fireEvent.click(await screen.findByText("Fixture Codex · codex · 版本 1"));
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "应用会话连接" })),
  );
  expect(f.c.selectedConnectionId()).toBe("cfg");
  expect(f.c.state.preferences.defaultConnectionId).toBe("B");
  expect(f.client.selectConnection).not.toHaveBeenCalled();
  expect(f.submit).not.toHaveBeenCalled();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "完成" })),
  );
  await act(async () => {
    f.workspace.navigation.select("assistant");
    f.app.rerender(
      <UiProvider>
        <AssistantPage workspace={f.workspace} />
      </UiProvider>,
    );
  });
  const input = screen.getByRole("textbox", { name: "消息" });
  expect((input as HTMLTextAreaElement).value).toBe("未发送的用户草稿");
  await act(async () =>
    fireEvent.keyDown(input, { key: "Enter", keyCode: 13 }),
  );
  expect(f.submit).toHaveBeenCalledOnce();
  expect(f.client.createSession).toHaveBeenLastCalledWith({
    sessionId: expect.any(String),
    connectionId: "cfg",
  });
});
it("renders the frozen history preview, requires confirmation, then shows and removes the one-shot attachment in the real AI page", async () => {
  const f = await fixture();
  fireEvent.click(screen.getByRole("button", { name: "会话选项" }));
  expect(
    (
      screen.getByRole("button", {
        name: "预览已完成文本",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await act(async () =>
    fireEvent.click(
      screen.getByRole("button", { name: "下次输入开始新上下文" }),
    ),
  );
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "预览已完成文本" })),
  );
  expect(screen.getByText("稳定原文，不包含工具或秘密")).toBeTruthy();
  expect(f.c.state.history.size).toBe(0);
  expect(f.submit).not.toHaveBeenCalled();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "确认带入这些文本" })),
  );
  expect(f.c.state.history.has(f.c.state.selected)).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "完成" })),
  );
  await act(async () =>
    f.app.rerender(
      <UiProvider>
        <AssistantPage workspace={f.workspace} />
      </UiProvider>,
    ),
  );
  expect(screen.getByText("下一条输入将带入已确认历史")).toBeTruthy();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "移除待带入历史" })),
  );
  expect(f.c.state.history.size).toBe(0);
  expect(f.submit).not.toHaveBeenCalled();
});
it("explains recovery_required and calls only explicit resume for the original session", async () => {
  const f = await fixture(),
    id = f.c.state.selected;
  await act(async () => {
    const v = f.c.state.views.get(id)!;
    v.sessionStatus = "recovery_required";
    v.capabilities.continuation = "across_processes";
  });
  fireEvent.click(screen.getByRole("button", { name: "会话选项" }));
  expect(screen.getByText("原上下文需要恢复")).toBeTruthy();
  expect(f.client.resume).not.toHaveBeenCalled();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "读取原会话状态" })),
  );
  expect(f.client.resume).not.toHaveBeenCalled();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "恢复原模型上下文" })),
  );
  expect(f.client.resume).toHaveBeenCalledExactlyOnceWith(id);
  expect(f.submit).not.toHaveBeenCalled();
  await waitFor(() => expect(f.c.view()!.sessionStatus).toBe("active"));
});

it("lets a recovering conversation explicitly start fresh after switching its pending connection, without replaying any prompt", async () => {
  const f = await fixture();
  await act(async () => {
    f.c.state.views.get(f.c.state.selected)!.sessionStatus =
      "recovery_required";
  });
  fireEvent.click(screen.getByRole("button", { name: "会话选项" }));
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "本会话连接" }));
  fireEvent.click(await screen.findByText("第二连接 · codex · 版本 1"));
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "应用会话连接" })),
  );
  expect(f.c.view()!.connectionPending).toBe(true);
  expect(f.c.view()!.sessionStatus).toBe("recovery_required");
  const fresh = screen.getByRole("button", { name: "下次输入开始新上下文" });
  expect((fresh as HTMLButtonElement).disabled).toBe(false);
  await act(async () => fireEvent.click(fresh));
  expect(f.client.selectConnection).toHaveBeenLastCalledWith(
    f.c.state.selected,
    "B",
    true,
  );
  expect(f.submit).not.toHaveBeenCalled();
  expect(f.client.resume).not.toHaveBeenCalled();
});
