import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useConversationScroll } from "./scroll";
afterEach(() => vi.restoreAllMocks());
it("follows output only at the bottom, preserves each session's reading position and releases late resize callbacks", () => {
  let resized = () => {};
  const disconnect = vi.fn();
  const previous = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class {
    constructor(callback: ResizeObserverCallback) {
      resized = () => callback([], this);
    }
    observe() {}
    unobserve() {}
    disconnect = disconnect;
  };
  const height = vi
      .spyOn(HTMLElement.prototype, "scrollHeight", "get")
      .mockReturnValue(2000),
    clientHeight = vi
      .spyOn(HTMLElement.prototype, "clientHeight", "get")
      .mockReturnValue(500);
  function Probe({ session, version }: { session: string; version: string }) {
    const stream = useConversationScroll(session, version, true);
    return (
      <div>
        <div ref={stream.viewport} data-testid="viewport">
          <div ref={stream.content}>content</div>
        </div>
        {stream.newOutput && <button onClick={stream.latest}>最新</button>}
      </div>
    );
  }
  const app = render(<Probe session="a" version="1" />),
    viewport = screen.getByTestId("viewport");
  expect(viewport.scrollTop).toBe(2000);
  act(() => {
    viewport.scrollTop = 120;
    fireEvent.scroll(viewport);
  });
  app.rerender(<Probe session="a" version="2" />);
  expect(viewport.scrollTop).toBe(120);
  expect(screen.getByRole("button", { name: "最新" })).toBeTruthy();
  app.rerender(<Probe session="b" version="1" />);
  expect(viewport.scrollTop).toBe(2000);
  app.rerender(<Probe session="a" version="2" />);
  expect(viewport.scrollTop).toBe(120);
  app.rerender(<Probe session="a" version="3" />);
  act(() => fireEvent.click(screen.getByRole("button", { name: "最新" })));
  expect(viewport.scrollTop).toBe(2000);
  expect(screen.queryByRole("button", { name: "最新" })).toBeNull();
  const late = resized;
  app.unmount();
  expect(disconnect).toHaveBeenCalledTimes(3);
  act(late);
  height.mockRestore();
  clientHeight.mockRestore();
  globalThis.ResizeObserver = previous;
});
