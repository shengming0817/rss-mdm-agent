import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Command } from "@rss-mdm-agent/ai-contract/wire";
import type { ClientOptions } from "@rss-mdm-agent/ai-client";
import { createWorkspace } from "../state/workspace";
import { shellFixture } from "../../../../tests/desktop/ui-support";
import { AssistantPage } from "./AssistantPage";
const releases: (() => void)[] = [];
afterEach(() => {
  for (const release of releases.splice(0)) release();
});
async function fixture() {
  const f = shellFixture();
  let options!: ClientOptions;
  const connect = f.input.assistantServices!.connect;
  f.input.assistantServices!.connect = async (input, signal) => {
    options = input;
    return connect(input, signal);
  };
  const workspace = createWorkspace(f.current(), f.input),
    c = workspace.assistant;
  await c.connect();
  c.create();
  c.draft.set("初始化会话");
  await c.prompt();
  f.submit.mockClear();
  const view = c.view()!;
  c.state.views.get(c.state.selected)!.selectedConnectionId = "cfg";
  workspace.navigation.select("assistant");
  const app = render(<AssistantPage workspace={workspace} />);
  releases.push(() => {
    app.unmount();
    workspace.dispose();
  });
  return { ...f, workspace, c, app, view, options };
}
it("uses the secondary history entry, keeps it open through selection/outside clicks and keeps per-session drafts through layout changes", async () => {
  const f = await fixture(),
    first = f.c.state.selected;
  expect(
    screen.queryByRole("complementary", { name: "历史会话清单" }),
  ).toBeNull();
  expect(f.app.container.querySelector(".rss-page-header")).toBeNull();
  expect(
    screen.queryByRole("button", { name: /固定|新建会话|历史会话$/ }),
  ).toBeNull();
  await act(async () => {
    f.c.draft.set("第一条草稿");
    f.workspace.navigation.toggleHistory();
  });
  const panel = screen.getByRole("complementary", { name: "历史会话清单" });
  const second = "history-second";
  await act(async () => {
    f.c.state.sessions.set(second, {
      namespace: { ...f.view.namespace, sessionId: second },
      status: "active",
      presentation: { title: "另一条会话" },
      lastActivityAtMs: 123,
    });
  });
  await act(async () =>
    fireEvent.change(
      within(panel).getByRole("textbox", { name: "搜索历史会话" }),
      { target: { value: "另一条" } },
    ),
  );
  expect(
    within(panel)
      .getAllByRole("button")
      .some((button) => button.textContent?.includes("另一条会话")),
  ).toBe(true);
  await act(async () =>
    fireEvent.click(within(panel).getByRole("button", { name: /另一条会话/ })),
  );
  expect(f.workspace.navigation.state.historyOpen).toBe(true);
  expect(f.c.state.selected).toBe(second);
  expect(
    (screen.getByRole("textbox", { name: "消息" }) as HTMLTextAreaElement)
      .value,
  ).toBe("");
  await act(async () => {
    f.c.draft.set("第二条草稿");
    fireEvent.click(screen.getByRole("region", { name: "AI 会话" }));
    f.workspace.navigation.cycle();
  });
  expect(f.workspace.navigation.state.historyOpen).toBe(true);
  await act(async () => f.c.select(first));
  expect(
    (screen.getByRole("textbox", { name: "消息" }) as HTMLTextAreaElement)
      .value,
  ).toBe("第一条草稿");
  await act(async () =>
    fireEvent.click(
      within(panel).getByRole("button", { name: "关闭历史会话" }),
    ),
  );
  expect(f.workspace.navigation.state.historyOpen).toBe(false);
  expect(f.c.draft.get()).toBe("第一条草稿");
});
it("IME Enter does not send, Shift Enter remains editing and a committed Enter uses the existing authenticated command", async () => {
  const f = await fixture(),
    input = screen.getByRole("textbox", { name: "消息" });
  await act(async () =>
    fireEvent.change(input, { target: { value: "中文输入" } }),
  );
  await act(async () => {
    fireEvent.compositionStart(input);
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
    fireEvent.compositionEnd(input);
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  });
  expect(f.submit).not.toHaveBeenCalled();
  expect(f.c.canSend()).toBe(true);
  expect(f.c.draft.get()).toBe("中文输入");
  await act(async () =>
    fireEvent.keyDown(input, { key: "Enter", keyCode: 13 }),
  );
  expect(f.submit).toHaveBeenCalledOnce();
  expect((f.submit.mock.calls as unknown as unknown[][])[0][0]).toMatchObject({
    input: { type: "prompt", text: "中文输入" },
  });
});
it("uses exact plain question keys, keeps the answer through page remount and exposes only one-request permissions for the selected session", async () => {
  const f = await fixture(),
    id = f.c.state.selected,
    generation = f.view.generation;
  await act(async () => {
    const view = f.c.state.views.get(id)!;
    view.capabilities.structuredQuestion = "supported";
    view.interactions.q = {
      interactionId: "q",
      commandId: "original-question",
      callbackLifetime: "generation_bound",
      generation,
      status: "pending",
      expiresAtMs: Date.now() + 60000,
      request: {
        questions: [
          {
            question: "请选择目标",
            header: "目标",
            options: [
              { label: "当前设备", description: "本次" },
              { label: "其它设备", description: "不变更" },
            ],
            multiSelect: false,
          },
        ],
      },
    } as (typeof view.interactions)[string];
    view.timeline.push({ kind: "interaction", key: "q", sequence: 1 });
  });
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "请选择目标" }), {
      target: { value: "用户明确回答" },
    }),
  );
  f.app.unmount();
  const app = render(<AssistantPage workspace={f.workspace} />);
  releases.push(() => app.unmount());
  expect(
    (screen.getByRole("textbox", { name: "请选择目标" }) as HTMLTextAreaElement)
      .value,
  ).toBe("用户明确回答");
  const abort = new AbortController(),
    backgroundAbort = new AbortController();
  let foreground!: Promise<unknown>, background!: Promise<unknown>;
  const request = (sessionId: string) => ({
    sessionId,
    toolCall: {
      toolCallId: "tool",
      title: sessionId === id ? "当前工具" : "后台工具",
    },
    options: [
      { optionId: "one", name: "本次", kind: "allow_once" as const },
      { optionId: "all", name: "总是", kind: "allow_always" as const },
      { optionId: "no", name: "拒绝", kind: "reject_once" as const },
    ],
  });
  await act(async () => {
    foreground = f.options.requestPermission!(request(id), abort.signal);
    background = f.options.requestPermission!(
      request("other"),
      backgroundAbort.signal,
    );
  });
  expect(screen.getByText("当前工具")).toBeTruthy();
  expect(screen.queryByText("后台工具")).toBeNull();
  expect(screen.queryByRole("button", { name: /始终|总是/ })).toBeNull();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "仅允许本次请求" })),
  );
  expect(await foreground).toEqual({
    outcome: { outcome: "selected", optionId: "one" },
  });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "提交本次回答" })),
  );
  expect((f.submit.mock.calls as unknown as unknown[][])[0][0]).toMatchObject({
    input: {
      type: "respond",
      interactionId: "q",
      answer: { answers: { 请选择目标: "用户明确回答" } },
    },
  });
  await act(async () => backgroundAbort.abort());
  await background;
});
it("retires surface input and keeps the main composer read-only when the original Host is disconnected", async () => {
  const f = await fixture(),
    memory = f.c.surfaceDrafts.form("private");
  await act(async () => {
    f.c.draft.set("私人草稿");
    memory.edit("answer", "私人回答");
    f.c.state.connection = "disconnected";
  });
  expect(
    (screen.getByRole("button", { name: "发送消息" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("textbox", { name: "消息" }) as HTMLTextAreaElement)
      .value,
  ).toBe("私人草稿");
  await act(async () => f.workspace.dispose());
  expect(memory.retired).toBe(true);
  expect(memory.fields.size).toBe(0);
  expect(f.c.state.drafts.size).toBe(0);
});

it("renders original prompt identity and projection completion state for the real timeline oracle", async () => {
  const f = await fixture();
  await act(async () => {
    f.c.draft.set("本次原消息");
    await f.c.prompt();
  });
  const command = (f.submit.mock.calls as unknown as [Command][])[0][0];
  await act(async () => {
    const view = f.c.state.views.get(f.c.state.selected)!;
    view.commands[command.commandId] = { command, state: "accepted" };
    view.messages.reply = {
      commandId: command.commandId,
      text: "本次回复",
      stable: false,
    };
    view.timeline = [
      { kind: "prompt", key: command.commandId, sequence: 1 },
      { kind: "message", key: "reply", sequence: 2 },
    ];
  });
  const timeline = f.app.container.querySelector(".rss-assistant-timeline")!;
  expect(timeline.getAttribute("data-busy")).toBe("true");
  const user = timeline.querySelector('[data-message-kind="user"]')!;
  expect(user.getAttribute("data-message-key")).toBe(command.commandId);
  expect(user.getAttribute("data-stable")).toBe("true");
  const reply = timeline.querySelector('[data-message-key="reply"]')!;
  expect(reply.getAttribute("data-stable")).toBe("false");
  await act(async () => {
    const view = f.c.state.views.get(f.c.state.selected)!;
    view.messages.reply.stable = true;
    view.commands[command.commandId].state = "terminal";
  });
  expect(reply.getAttribute("data-stable")).toBe("true");
  expect(timeline.getAttribute("data-busy")).toBe("false");
});
