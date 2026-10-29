import { expect, it, vi } from "vitest";
import { createConnectionSettings } from "./connections";
import type { AssistantController } from "../assistant/controller";
import type { ConnectionDraft } from "@rss-mdm-agent/ai-contract/wire";
it("sends a secret only to the native saver with the frozen revision and never publishes it or raw errors", async () => {
  const secret = "private-secret-never-render",
    save = vi.fn().mockRejectedValue(Error(secret)),
    refreshConnections = vi.fn();
  const owner = createConnectionSettings(
    {
      runtime: { current: undefined },
      refreshConnections,
    } as unknown as AssistantController,
    save,
  );
  const draft: ConnectionDraft = {
    connectionId: "cfg",
    provider: "deepseek",
    profile: "conversation",
    source: {
      type: "custom_api",
      apiUrl: "https://example.test",
      model: "fixed",
    },
    presentation: { name: "Work" },
  };
  await owner.save(draft, 7, secret);
  expect(save).toHaveBeenCalledExactlyOnceWith(draft, 7, secret);
  expect(JSON.stringify(owner.state)).not.toContain(secret);
  expect(refreshConnections).not.toHaveBeenCalled();
  owner.dispose();
});
it("suppresses duplicate writes and ignores an old workspace's save completion", async () => {
  let finish!: (value: never) => void;
  const save = vi.fn(
      () =>
        new Promise<never>((resolve) => {
          finish = resolve;
        }),
    ),
    refreshConnections = vi.fn();
  const owner = createConnectionSettings(
    {
      runtime: { current: undefined },
      refreshConnections,
    } as unknown as AssistantController,
    save,
  );
  const draft = { connectionId: "cfg" } as ConnectionDraft;
  const pending = owner.save(draft, null);
  await owner.save(draft, null);
  expect(save).toHaveBeenCalledTimes(1);
  owner.dispose();
  finish(undefined as never);
  await pending;
  expect(refreshConnections).not.toHaveBeenCalled();
  expect(owner.state.message).toBe("");
});
