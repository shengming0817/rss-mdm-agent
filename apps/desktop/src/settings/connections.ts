import { proxy } from "valtio/vanilla";
import type {
  Connection,
  ConnectionDraft,
} from "@rss-mdm-agent/ai-contract/wire";
import type { AssistantController } from "../assistant/controller";
import type { ConnectionSavePort } from "./native";

/** Connection metadata remains in the assistant owner; secrets never enter reactive state. */
export function createConnectionSettings(
  assistant: AssistantController,
  savePort?: ConnectionSavePort,
) {
  const state = proxy({ busy: false, message: "" });
  let active = true;
  async function run(operation: () => Promise<unknown>, success: string) {
    if (!active || state.busy) return false;
    state.busy = true;
    state.message = "";
    try {
      await operation();
      if (!active) return false;
      await assistant.refreshConnections();
      if (!active) return false;
      state.message = success;
      return true;
    } catch {
      if (active)
        state.message =
          "连接操作未确认。请重读当前配置后再操作；不会自动重发或回显凭据。";
      return false;
    } finally {
      if (active) state.busy = false;
    }
  }
  return {
    state,
    canSave: !!savePort,
    save(draft: ConnectionDraft, expected: number | null, secret?: string) {
      if (!savePort) return Promise.resolve(false);
      const pending = run(() => {
        const value = secret;
        secret = undefined;
        return savePort(draft, expected, value);
      }, "连接已保存，验证后才能用于会话。");
      secret = undefined;
      return pending;
    },
    test(connection: Connection) {
      const client = assistant.runtime.current;
      if (!client) return Promise.resolve(false);
      return run(
        () =>
          client.testConnection(
            connection.connectionId,
            connection.configRevision,
          ),
        "已读取连接验证结果。",
      );
    },
    remove(connection: Connection) {
      const client = assistant.runtime.current;
      if (!client) return Promise.resolve(false);
      return run(
        () =>
          client.deleteConnection(
            connection.connectionId,
            connection.configRevision,
          ),
        "连接已移除。",
      );
    },
    refresh() {
      return run(() => assistant.refreshConnections(), "已重读连接配置。");
    },
    dispose() {
      active = false;
      state.busy = false;
      state.message = "";
    },
  };
}
