import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { proxy } from "valtio/vanilla";
import { UiProvider } from "@rss-mdm-agent/ui";
import type { Connection } from "@rss-mdm-agent/ai-contract/wire";
import type { AssistantController } from "../assistant/controller";
import type { Workspace } from "../state/workspace";
import { createConnectionSettings } from "./connections";
import { ConnectionsPage } from "./ConnectionsPage";
function mount() {
  const connection: Connection = {
    schemaVersion: 1,
    kind: "connection",
    connectionId: "work",
    configRevision: 1,
    status: "ready",
    provider: "codex",
    profile: "conversation",
    source: { type: "existing_config" },
    presentation: { name: "Work" },
  };
  const state = proxy({ connection: "connected", connections: [connection] });
  const testConnection = vi.fn(async () => {
    state.connections[0].configRevision++;
  });
  const deleteConnection = vi.fn(async () => {
    state.connections = [];
  });
  const save = vi.fn(async () => connection);
  const assistant = {
    state,
    runtime: { current: { testConnection, deleteConnection } },
    refreshConnections: vi.fn(async () => {}),
  } as unknown as AssistantController;
  const connections = createConnectionSettings(assistant, save);
  render(
    <UiProvider reducedMotion>
      <ConnectionsPage
        workspace={{ assistant, connections } as Workspace}
        active
      />
    </UiProvider>,
  );
  return { state, testConnection, deleteConnection, save, connections };
}
it("ends validation's editor and uses the refreshed revision for a subsequent save and removal", async () => {
  const f = mount();
  fireEvent.click(screen.getByRole("button", { name: "配置 Work" }));
  fireEvent.click(screen.getByRole("button", { name: "验证当前已保存配置" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(f.testConnection).toHaveBeenCalledExactlyOnceWith("work", 1);
  fireEvent.click(screen.getByRole("button", { name: "配置 Work" }));
  fireEvent.click(screen.getByRole("button", { name: "保存连接" }));
  await waitFor(() =>
    expect(f.save).toHaveBeenCalledWith(expect.anything(), 2, undefined),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "配置 Work" }));
  fireEvent.click(screen.getByRole("button", { name: "移除连接" }));
  fireEvent.click(
    within(screen.getByRole("tooltip")).getByRole("button", { name: /确/ }),
  );
  await waitFor(() =>
    expect(f.deleteConnection).toHaveBeenCalledExactlyOnceWith("work", 2),
  );
  f.connections.dispose();
});
it("offers recoverable feedback when metadata changes while an editor is open", async () => {
  const f = mount();
  fireEvent.click(screen.getByRole("button", { name: "配置 Work" }));
  act(() => {
    f.state.connections[0].configRevision = 2;
  });
  fireEvent.click(screen.getByRole("button", { name: "验证当前已保存配置" }));
  expect(
    screen.getByText("此配置已变化或移除，请关闭并重新打开当前配置后继续。"),
  ).toBeTruthy();
  expect(f.testConnection).not.toHaveBeenCalled();
  f.connections.dispose();
});
