import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import {
  RuntimeSurface,
  type NodeMount,
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
import {
  getValue,
  ResolvedBinding,
  type ComponentNode,
} from "@a2ui/web_core/v0_9";
import { SurfaceRenderer } from "../../../../packages/ai-ui-bridge/dist/renderer.js";

afterEach(() => vi.useRealTimers());
// Only the product-specific node mapping is minimal here. Resolution, action binding,
// pending identity, deadlines and retry policy run through the real official renderer.
const mount: NodeMount = (container, root, enabled) => {
  function draw(node: ComponentNode): HTMLElement {
    const props = getValue(node.props);
    if (node.type === "Button") {
      const button = document.createElement("button");
      button.type = "button";
      button.disabled = !enabled;
      if (props.child) button.append(draw(props.child as ComponentNode));
      button.onclick = () => void (props.action as () => void)();
      return button;
    }
    if (node.type === "TextField") {
      const input = document.createElement("input");
      input.setAttribute("aria-label", String(props.label));
      input.disabled = !enabled;
      return input;
    }
    const element = document.createElement("div");
    if (node.type === "Text")
      element.textContent = String(
        props.text instanceof ResolvedBinding ? props.text.value : props.text,
      );
    else
      for (const child of (props.children ?? []) as ComponentNode[])
        element.append(draw(child));
    return element;
  }
  const node = draw(root);
  container.append(node);
  return () => node.remove();
};
async function fixture() {
  const seeded = await seedSurface(new MemorySessionStore());
  const instanceId = seeded.surface.surfaceInstanceId,
    interactionId = seeded.interaction.interactionId;
  let clock = 0,
    rejectAction: (error: unknown) => void = () => {};
  const now = () => clock;
  const view = {
    namespace: seeded.session.namespace,
    generation: seeded.surface.generation,
    connection: "attached",
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
            now={now}
            rendererFactory={rendererFactory}
            onError={(error) => errors.push(error)}
          />,
        );
      });
      return mounted;
    },
    factory,
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
  expect(screen.queryByText(/Response could not be confirmed/)).toBeNull();
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
  expect(screen.getByText(/This question can no longer accept/)).toBeTruthy();
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
