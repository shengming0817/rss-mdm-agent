import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useState } from "react";
import {
  ActionStatus,
  DetailDrawer,
  MessageComposer,
  MessageStream,
  UiProvider,
  StatePanel,
} from "../src";

describe("product presentation", () => {
  it("does not turn ended, unknown or cancellation requested into verified success", () => {
    render(
      <UiProvider>
        <ActionStatus phase="ended" />
        <ActionStatus phase="unknown" />
        <ActionStatus phase="cancelRequested" />
      </UiProvider>,
    );
    expect(screen.getByText("执行结束，待核实")).toBeTruthy();
    expect(screen.getByText("结果未知")).toBeTruthy();
    expect(screen.getByText("已请求取消")).toBeTruthy();
    expect(screen.queryByText("核实成功")).toBeNull();
  });
  it("only queries the original request on unknown results", () => {
    const read = vi.fn();
    render(
      <UiProvider>
        <StatePanel kind="unknown" title="结果未知" onRead={read} />
      </UiProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "查询原请求" }));
    expect(read).toHaveBeenCalledOnce();
  });
  it("ignores Enter during IME and keeps Shift+Enter as a newline", () => {
    const send = vi.fn();
    function Form() {
      const [value, setValue] = useState("草稿");
      return (
        <UiProvider>
          <MessageComposer value={value} onChange={setValue} onSend={send} />
        </UiProvider>
      );
    }
    render(<Form />);
    const input = screen.getByRole("textbox", { name: "消息" });
    fireEvent.compositionStart(input);
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.compositionEnd(input);
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(send).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(send).toHaveBeenCalledOnce();
  });
  it("renders untrusted markdown without executable HTML, navigation or resource loading", () => {
    const copy = vi.fn();
    const { container } = render(
      <UiProvider>
        <MessageStream
          onCopy={copy}
          messages={[
            {
              id: "one",
              kind: "assistant",
              stable: true,
              text: "<script>alert(1)</script>\n\n![tracking](https://example.invalid/pixel) [site](https://example.invalid)\n\n```sh\necho safe\n```",
            },
          ]}
        />
      </UiProvider>,
    );
    expect(container.querySelector("script,img,a,iframe")).toBeNull();
    expect(screen.getByText(/图片：tracking/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "复制链接" }));
    expect(copy).toHaveBeenCalledWith("https://example.invalid");
  });
  it("keeps an open drawer and controlled drafts when switching theme", () => {
    const close = vi.fn();
    const { rerender } = render(
      <UiProvider theme="light">
        <DetailDrawer open onClose={close} title="详情">
          <input aria-label="草稿" defaultValue="保留" />
        </DetailDrawer>
      </UiProvider>,
    );
    fireEvent.change(screen.getByRole("textbox", { name: "草稿" }), {
      target: { value: "编辑后" },
    });
    rerender(
      <UiProvider theme="dark">
        <DetailDrawer open onClose={close} title="详情">
          <input aria-label="草稿" defaultValue="保留" />
        </DetailDrawer>
      </UiProvider>,
    );
    expect(
      (screen.getByRole("textbox", { name: "草稿" }) as HTMLInputElement).value,
    ).toBe("编辑后");
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(close).not.toHaveBeenCalled();
  });
});
