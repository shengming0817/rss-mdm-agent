import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  RuntimeSurface,
  SurfaceDrafts,
  type RendererFactory,
  type RendererError,
} from "@rss-mdm-agent/ai-ui-bridge";
import {
  ClientError,
  type RuntimeClient,
  type SessionView,
  type InteractionView,
} from "@rss-mdm-agent/ai-client";
import type { ActionRequest, Receipt } from "@rss-mdm-agent/ai-contract/wire";
import {
  MemorySessionStore,
  seedSurface,
} from "@rss-mdm-agent/ai-contract/testing";
import { SurfaceRenderer } from "../../../../packages/ai-ui-bridge/dist/renderer.js";

afterEach(() => vi.useRealTimers());
// Real production React nodes and the real official core share this behavior carrier.
import { mountQuestionNode as mount } from "./operation-ui/nodes";
async function fixture() {
  const seeded = await seedSurface(new MemorySessionStore());
  const drafts = new SurfaceDrafts();
  const instanceId = seeded.surface.surfaceInstanceId,
    interactionId = seeded.interaction.interactionId;
  let clock = 0,
    rejectAction: (error: unknown) => void = () => {};
  const now = () => clock;
  const view = {
    namespace: seeded.session.namespace,
    generation: seeded.surface.generation,
    connection: "attached",
    sessionStatus: "active",
    surfaces: { [instanceId]: seeded.surface },
    interactions: { [interactionId]: seeded.interaction },
  } as unknown as SessionView;
  const listeners = new Set<(view: SessionView) => void>();
  const action = vi.fn(
    (_request: ActionRequest) =>
      new Promise<Receipt>((_resolve, reject) => {
        rejectAction = reject;
      }),
  );
  const runtime = {
    getSession: () => structuredClone(view),
    observe: (listener: (view: SessionView) => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    action,
    restore: vi.fn(async () => structuredClone(view)),
  } as unknown as RuntimeClient;
  const errors: RendererError[] = [];
  const factory: RendererFactory = async (container, options) =>
    new SurfaceRenderer(container, options);
  function notify(patch: Partial<InteractionView> = {}) {
    Object.assign(view.interactions[interactionId], patch);
    for (const listener of listeners) listener(structuredClone(view));
  }
  return {
    view,
    instanceId,
    action,
    errors,
    notify,
    time: (value: number) => {
      clock = value;
    },
    reject: (error: unknown) => rejectAction(error),
    async mount(rendererFactory = factory) {
      let mounted!: ReturnType<typeof render>;
      await act(async () => {
        mounted = render(
          <RuntimeSurface
            runtime={runtime}
            sessionId={view.namespace.sessionId}
            instanceId={instanceId}
            mount={mount}
            drafts={drafts}
            now={now}
            rendererFactory={rendererFactory}
            onError={(error) => errors.push(error)}
          />,
        );
      });
      return mounted;
    },
    factory,
    drafts,
  };
}
it("honors the inclusive deadline and expires the real card without a server event", async () => {
  const f = await fixture();
  vi.useFakeTimers();
  const view = await f.mount();
  expect(
    (screen.getByRole("button", { name: "Send answer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  act(() => {
    f.time(100);
    f.notify();
  });
  expect(
    (screen.getByRole("button", { name: "Send answer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
  await act(async () => {
    f.time(101);
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(
    (screen.getByRole("button", { name: "Send answer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.getByText("此问题已过期。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
  view.unmount();
  expect(vi.getTimerCount()).toBe(0);
});
it("retries the same original answer and removes retry when another client wins", async () => {
  const f = await fixture();
  await f.mount();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Send answer" })),
  );
  expect(f.action).toHaveBeenCalledOnce();
  expect(f.action.mock.calls[0][0].expiresAtMs).toBe(100);
  await act(async () => f.reject(new ClientError("transport_failed")));
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "重试原回答" })),
  );
  expect(f.action).toHaveBeenCalledTimes(2);
  expect(f.action.mock.calls[1][0]).toEqual(f.action.mock.calls[0][0]);
  act(() =>
    f.notify({ status: "answered", responseCommandId: "other-client" }),
  );
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
  await act(async () => f.reject(new ClientError("transport_failed")));
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
  expect(screen.queryByText(/原回答回执未能确认/)).toBeNull();
});
it("does not revive an unavailable callback after a late rejection", async () => {
  const f = await fixture();
  await f.mount();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Send answer" })),
  );
  act(() => f.notify({ status: "unavailable" }));
  await act(async () => f.reject(new ClientError("already_answered")));
  expect(screen.getByText("此问题已不可用。")).toBeTruthy();
  expect(
    (screen.getByRole("button", { name: "Send answer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
});
it("reports terminal rejection without offering retry before a winning notification", async () => {
  const f = await fixture();
  await f.mount();
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Send answer" })),
  );
  await act(async () => f.reject(new ClientError("already_answered")));
  expect(screen.getAllByText(/原问题不能再接受此回答/).length).toBeGreaterThan(
    0,
  );
  expect(
    (screen.getByRole("button", { name: "Send answer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
  expect(f.errors.at(-1)).toMatchObject({
    code: "action_rejected",
    failure: "already_answered",
  });
});
it("recovers load and real renderer validation failures using the same projection", async () => {
  const f = await fixture();
  let unavailable = true;
  await f.mount(async (container, options) => {
    if (unavailable) throw Error("load");
    return f.factory(container, options);
  });
  expect(screen.getByRole("button", { name: "重新加载卡片" })).toBeTruthy();
  unavailable = false;
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "重新加载卡片" })),
  );
  expect(screen.getByRole("button", { name: "Send answer" })).toBeTruthy();
  const original = f.view.surfaces[f.instanceId];
  f.view.surfaces[f.instanceId] = {
    ...original,
    revision: original.revision + 1,
    messages: [],
  };
  act(() => f.notify());
  expect(screen.getByRole("button", { name: "重新加载卡片" })).toBeTruthy();
  f.view.surfaces[f.instanceId] = original;
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "重新加载卡片" })),
  );
  expect(screen.getByRole("button", { name: "Send answer" })).toBeTruthy();
});

it("binds actual React input to the official action and recovers the frozen answer after a page remount", async () => {
  const f = await fixture(),
    app = await f.mount();
  const input = screen.getByRole("textbox", { name: "Answer" });
  act(() => fireEvent.change(input, { target: { value: "用户最终输入" } }));
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Send answer" })),
  );
  const original = structuredClone(f.action.mock.calls[0][0]);
  expect(JSON.stringify(original.message)).toContain("用户最终输入");
  await act(async () => f.reject(new ClientError("transport_failed")));
  await act(async () => app.unmount());
  await f.mount();
  expect(
    (screen.getByRole("textbox", { name: "Answer" }) as HTMLInputElement).value,
  ).toBe("用户最终输入");
  expect(
    (screen.getByRole("textbox", { name: "Answer" }) as HTMLInputElement)
      .disabled,
  ).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "重试原回答" })),
  );
  expect(f.action.mock.calls[1][0]).toEqual(original);
  await act(async () => f.reject(new ClientError("transport_failed")));
});
it("preserves compatible fields and focus across a newer official surface, then drops incompatible bindings", async () => {
  const f = await fixture();
  await f.mount();
  const input = screen.getByRole("textbox", {
    name: "Answer",
  }) as HTMLInputElement;
  act(() => {
    input.focus();
    fireEvent.change(input, { target: { value: "未提交草稿" } });
  });
  const surface = f.view.surfaces[f.instanceId];
  surface.revision++;
  await act(async () => f.notify());
  expect(screen.getByRole("textbox", { name: "Answer" })).toBe(input);
  expect(document.activeElement).toBe(input);
  expect(input.value).toBe("未提交草稿");
  const components = (surface.messages[1] as any).updateComponents.components;
  components.find((node: any) => node.id === "field").value.path =
    "/replacement";
  surface.revision++;
  await act(async () => f.notify());
  expect(input.value).toBe("");
  expect(screen.getByText(/不兼容的旧输入已清空/)).toBeTruthy();
  expect([...f.drafts.acquire(surface).fields.values()]).not.toContain(
    "未提交草稿",
  );
});
it("shares one answer barrier across two mounted copies and fences old revisions", async () => {
  const f = await fixture();
  await f.mount();
  await f.mount();
  const buttons = screen.getAllByRole("button", { name: "Send answer" });
  await act(async () => {
    fireEvent.click(buttons[0]);
    fireEvent.click(buttons[1]);
  });
  expect(f.action).toHaveBeenCalledOnce();
  await act(async () => f.reject(new ClientError("transport_failed")));
  f.view.surfaces[f.instanceId].revision++;
  await act(async () => f.notify());
  expect(screen.queryByRole("button", { name: "重试原回答" })).toBeNull();
  expect(screen.getAllByText(/问题版本已变化/).length).toBe(2);
  await act(async () => {
    for (const button of screen.getAllByRole("button", { name: "Send answer" }))
      fireEvent.click(button);
  });
  expect(f.action).toHaveBeenCalledOnce();
});
it("never persists or submits an obscured question field and retires all ordinary input on account disposal", async () => {
  const f = await fixture();
  const surface = f.view.surfaces[f.instanceId];
  (surface.messages[1] as any).updateComponents.components.find(
    (node: any) => node.id === "field",
  ).variant = "obscured";
  await f.mount();
  const input = screen.getByRole("textbox", {
    name: "Answer",
  }) as HTMLInputElement;
  expect(input.value).toBe("");
  expect(input.disabled).toBe(true);
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Send answer" })),
  );
  expect(f.action).not.toHaveBeenCalled();
  expect(f.drafts.acquire(surface).fields.size).toBe(0);
  const memory = f.drafts.acquire(surface);
  act(() => f.drafts.clear());
  expect(memory.retired).toBe(true);
  expect(memory.pending).toBeUndefined();
});
