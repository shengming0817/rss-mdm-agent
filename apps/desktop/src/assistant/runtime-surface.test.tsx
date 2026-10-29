import { useLayoutEffect, useRef } from "react";
import { act, render, screen } from "@testing-library/react";
import { it, expect, vi } from "vitest";
import {
  RuntimeSurface,
  useSessionView,
  type SurfaceRendererHandle,
  type RendererFactory,
} from "@rss-mdm-agent/ai-ui-bridge";
import type { RuntimeClient, SessionView } from "@rss-mdm-agent/ai-client";

function runtime() {
  const listeners = new Set<(view: SessionView) => void>();
  return {
    client: {
      getSession: () => undefined,
      observe: (listener: (view: SessionView) => void) => {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    } as unknown as RuntimeClient,
    listeners,
  };
}
const mount = () => () => {};
function handle(): SurfaceRendererHandle {
  return {
    canRetry: false,
    replace: vi.fn(),
    clear: vi.fn(),
    dispose: vi.fn(),
    retry: vi.fn(),
  };
}
it("disposes a renderer that resolves after unmount and never subscribes it", async () => {
  const r = runtime();
  const renderer = handle();
  let resolve!: (value: SurfaceRendererHandle) => void;
  const factory: RendererFactory = () =>
    new Promise((done) => {
      resolve = done;
    });
  const view = render(
    <RuntimeSurface
      runtime={r.client}
      sessionId="s"
      instanceId="i"
      mount={mount}
      rendererFactory={factory}
    />,
  );
  view.unmount();
  await act(async () => {
    resolve(renderer);
  });
  expect(renderer.dispose).toHaveBeenCalledOnce();
  expect(r.listeners.size).toBe(0);
});
it("replaces subscriptions and renderers when the authenticated owner changes", async () => {
  const first = runtime(),
    second = runtime();
  const old = handle(),
    next = handle();
  const factory = vi
    .fn()
    .mockResolvedValueOnce(old)
    .mockResolvedValueOnce(next);
  const view = render(
    <RuntimeSurface
      runtime={first.client}
      sessionId="s"
      instanceId="i"
      mount={mount}
      rendererFactory={factory}
    />,
  );
  await act(async () => {});
  expect(first.listeners.size).toBe(1);
  view.rerender(
    <RuntimeSurface
      runtime={second.client}
      sessionId="s"
      instanceId="i"
      mount={mount}
      rendererFactory={factory}
    />,
  );
  await act(async () => {});
  expect(first.listeners.size).toBe(0);
  expect(old.dispose).toHaveBeenCalledOnce();
  expect(second.listeners.size).toBe(1);
  view.unmount();
  expect(second.listeners.size).toBe(0);
  expect(next.dispose).toHaveBeenCalledOnce();
});
it("does not show the previous session projection during an owner switch", async () => {
  const first = runtime(),
    second = runtime();
  function View({ client }: { client: RuntimeClient }) {
    const view = useSessionView(client, "s");
    return <p>{view?.namespace.principalId ?? "empty"}</p>;
  }
  const view = render(<View client={first.client} />);
  act(() => {
    for (const listener of first.listeners)
      listener({
        namespace: { sessionId: "s", principalId: "old" },
      } as SessionView);
  });
  expect(screen.getByText("old")).toBeTruthy();
  view.rerender(<View client={second.client} />);
  expect(screen.queryByText("old")).toBeNull();
  expect(first.listeners.size).toBe(0);
});

it("removes the previous owner's private card before the next owner's layout and blocks stale actions", async () => {
  const first = runtime(),
    second = runtime();
  const action = vi.fn();
  first.client.action = action;
  let stale: Parameters<RendererFactory>[1]["onAction"] | undefined;
  let observed = "";
  const factory: RendererFactory = async (container, options) => {
    if (!stale) {
      container.textContent = "OLD OWNER PRIVATE TEXT";
      stale = options.onAction;
    }
    return handle();
  };
  function Parent({ client }: { client: RuntimeClient }) {
    const root = useRef<HTMLElement>(null);
    useLayoutEffect(() => {
      observed = root.current?.textContent ?? "";
    });
    return (
      <section ref={root}>
        <RuntimeSurface
          runtime={client}
          sessionId="s"
          instanceId="i"
          mount={mount}
          rendererFactory={factory}
        />
      </section>
    );
  }
  const view = render(<Parent client={first.client} />);
  await act(async () => {});
  expect(observed).toContain("OLD OWNER PRIVATE TEXT");
  view.rerender(<Parent client={second.client} />);
  expect(observed).not.toContain("OLD OWNER PRIVATE TEXT");
  await act(async () => {
    await stale!({} as Parameters<NonNullable<typeof stale>>[0]);
  });
  expect(action).not.toHaveBeenCalled();
});
