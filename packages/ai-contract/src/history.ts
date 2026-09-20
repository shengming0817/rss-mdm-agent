import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex } from "@noble/hashes/utils.js";
import canonicalize from "canonicalize";
import type {
  CommandRecord,
  Connection,
  Event,
  HistoryPreview,
  Session,
} from "./wire.js";
import { boundedJson } from "./codec.js";
import { defaultLimits } from "./results.js";

/** Deliberately plain text: only completed user prompts and stable assistant messages. */
export function historyPreview(
  session: Session,
  commands: readonly CommandRecord[],
  events: readonly Event[],
  connection: Connection,
  recent?: number,
): HistoryPreview {
  if (
    recent !== undefined &&
    (!Number.isInteger(recent) || recent < 1 || recent > 10000)
  )
    throw new Error("invalid_input");
  const completed = commands
    .filter(
      (row) =>
        row.command.input.type === "prompt" &&
        row.state === "terminal" &&
        row.outcome === "completed",
    )
    .sort((a, b) => a.receipt.acceptedRevision - b.receipt.acceptedRevision);
  const selected = recent === undefined ? completed : completed.slice(-recent);
  const commandIds = selected.map((row) => row.command.commandId);
  const messageIds: string[] = [];
  const text = selected
    .map((row) => {
      const messages = new Map<string, string>();
      for (const event of events)
        if (
          event.commandId === row.command.commandId &&
          event.body.type === "text"
        )
          messages.set(event.body.messageId, event.body.text);
      messageIds.push(...messages.keys());
      return `User:\n${row.command.input.type === "prompt" ? row.command.input.text : ""}\n\nAssistant:\n${[...messages.values()].join("\n\n")}`;
    })
    .join("\n\n");
  const value = {
    schemaVersion: 5 as const,
    kind: "historyPreview" as const,
    sessionId: session.namespace.sessionId,
    connectionId: connection.connectionId,
    configRevision: connection.configRevision,

    throughSequence: session.lastSequence,
    commandIds,
    messageIds,
    text,
  };
  boundedJson(value, { ...defaultLimits, maxTextBytes: 65536 });
  return {
    ...value,
    contentHash: bytesToHex(
      sha256(new TextEncoder().encode(canonicalize(value)!)),
    ),
  };
}

/** The confirmed preview is part of the immutable command hash, not an implicit resume. */
export function promptText(
  input: Extract<import("./wire.js").Input, { type: "prompt" }>,
): string {
  return input.history
    ? `Previous completed conversation (user-confirmed plain text):\n${input.history.text}\n\nCurrent user request:\n${input.text}`
    : input.text;
}
