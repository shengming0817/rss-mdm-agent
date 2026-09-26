import type { RequestView } from "./types";
import preview from "./preview";
export function executionTask(): RequestView {
  const item = preview.catalog.find((i) => i.itemId === "office")!;
  return {
    status: "confirmation",
    message: "等待本人确认",
    interactions: [
      {
        id: "confirm-action",
        kind: { kind: "executionAction", digest: "a".repeat(64) },
        status: "pending",
        expiresAtUnixMs: 2000,
        message: "确认本次动作",
        options: [],
      },
    ],
    action: {
      riskLevel: 2,
      authority: { kind: "test", id: "test" },
      actor: "actor",
      initiator: {
        kind: "human",
        osSession: {
          device: "device",
          account: { platform: "macos", subject: "user" },
          session: "session",
        },
      },
      requestId: "request-1",
      revision: 1,
      digest: "a".repeat(64),
      itemId: item.itemId,
      title: item.name,
      action: "install",
      resource: item.resource,
      target: "device",
      runAs: "user",
      network: "denied",
      dataScope: "bounded",
      permission: "existing",
      parameters: [],
      expiresAtUnixMs: 2000,
    },
  };
}
