import { ClientError } from "./errors.js";
import {
  validateSurface,
  accessLimits,
  type CommandRecord,
  type Command,
  type Capabilities,
  type Failure,
  type Event,
  type Interaction,
  type Outcome,
  type Session,
  type SnapshotPage,
  type Subscription,
  type SurfaceState,
} from "@rss-mdm-agent/ai-contract";

export type InteractionView = Pick<
  Interaction,
  | "commandId"
  | "nativeRunId"
  | "status"
  | "request"
  | "expiresAtMs"
  | "callbackLifetime"
  | "responseCommandId"
  | "generation"
>;
const projectInteraction = (row: Interaction): InteractionView => ({
  commandId: row.commandId,
  ...(row.nativeRunId ? { nativeRunId: row.nativeRunId } : {}),
  status: row.status,
  request: row.request,
  expiresAtMs: row.expiresAtMs,
  callbackLifetime: row.callbackLifetime,
  generation: row.generation,
  ...(row.responseCommandId
    ? { responseCommandId: row.responseCommandId }
    : {}),
});
export interface CommandView {
  command: Command;
  state: CommandRecord["state"];
  dispatch?: CommandRecord["dispatch"];
  outcome?: Outcome;
  failure?: Failure;
  acknowledgement?: CommandRecord["acknowledgement"];
  cancelledBy?: string;
  cancelDispatched?: Extract<
    Event["body"],
    { type: "cancel_dispatched" }
  >["confirmation"];
}
export interface TimelineItem {
  kind: "prompt" | "message" | "tool" | "interaction" | "surface";
  key: string;
  sequence: number;
}
export interface SessionView {
  /** Identity of this attachment's snapshot, not a second live Host Session. */
  namespace: Session["namespace"];
  generation: string;
  cursor: number;
  capabilities: Capabilities;
  sessionStatus: Session["status"];
  timeline: TimelineItem[];
  connection: "attached" | "detached" | "resync_required";
  commands: Record<string, CommandView>;
  messages: Record<
    string,
    { commandId: string; text: string; stable: boolean }
  >;
  interactions: Record<string, InteractionView>;
  surfaces: Record<string, SurfaceState>;
  tools: Record<
    string,
    {
      commandId: string;
      proposalId: string;
      name: string;
      arguments: Record<string, unknown>;
      status: "pending" | "completed" | "failed";
      result?: Pick<
        Extract<Event["body"], { type: "tool_result" }>,
        "disposition" | "text"
      >;
    }
  >;
}
const messageKey = (command: string, message: string) =>
  JSON.stringify([command, message]);
export function emptyView(session: Session, cursor: number): SessionView {
  return {
    namespace: structuredClone(session.namespace),
    generation: session.binding.generation,
    cursor,
    capabilities: structuredClone(session.capabilities),
    sessionStatus: session.status,
    timeline: [],
    connection: "detached",
    commands: Object.create(null),
    messages: Object.create(null),
    interactions: Object.create(null),
    surfaces: Object.create(null),
    tools: Object.create(null),
  };
}
function order(
  view: SessionView,
  kind: TimelineItem["kind"],
  key: string,
  sequence: number,
) {
  const prior = view.timeline.find(
    (item) => item.kind === kind && item.key === key,
  );
  if (prior) {
    // A temporary block obtains its durable position only on a stable event.
    if (!Number.isInteger(prior.sequence)) prior.sequence = sequence;
  } else view.timeline.push({ kind, key, sequence });
  view.timeline.sort((a, b) => a.sequence - b.sequence);
}
function event(view: SessionView, e: Event): void {
  const body = e.body;
  if (body.type === "session_recovery_unavailable") {
    view.sessionStatus = "recovery_required";
    view.connection = "resync_required";
    return;
  }
  if (body.type === "session_retired") {
    view.sessionStatus = "retired";
    return;
  }
  if (e.commandId === undefined) return;
  if (body.type === "command_accepted") {
    if (view.commands[e.commandId]) throw new ClientError("resync_required");
    view.commands[e.commandId] = {
      command: structuredClone(body.command),
      state: "accepted",
    };
    if (body.command.input.type === "prompt")
      order(view, "prompt", e.commandId, e.sequence);
    return;
  }
  const command = view.commands[e.commandId];
  if (!command) throw new ClientError("resync_required");
  const settled = [
    "terminal",
    "invalidated",
    "acknowledged",
    "cancelled",
  ].includes(command.state);
  if (
    body.type === "terminal" ||
    body.type === "invalidated" ||
    body.type === "acknowledged" ||
    body.type === "cancelled"
  ) {
    if (settled) return;
    if (body.type === "acknowledged") {
      command.state = "acknowledged";
      command.acknowledgement = body.acknowledgement;
    } else if (body.type === "cancelled") {
      command.state = "cancelled";
      command.cancelledBy = body.cancelledBy;
    } else if (body.type === "terminal") {
      command.state = "terminal";
      command.outcome = body.outcome;
    } else {
      command.state = "invalidated";
      command.failure = body.failure;
    }
    for (const [key, message] of Object.entries(view.messages))
      if (message.commandId === e.commandId && !message.stable) {
        delete view.messages[key];
        view.timeline = view.timeline.filter(
          (item) => item.kind !== "message" || item.key !== key,
        );
      }
  } else if (body.type === "status") {
    if (!settled) command.state = body.state;
  } else if (body.type === "dispatch" || body.type === "reconciled") {
    if (settled) return;
    if (body.type === "reconciled" && body.resolution === "not_submitted") {
      command.state = "accepted";
      delete command.dispatch;
    } else command.dispatch = structuredClone(body.attempt);
  } else if (body.type === "cancel_dispatched") {
    command.cancelDispatched = body.confirmation;
  } else if (body.type === "error") {
    if (!settled) command.failure = body.failure;
  } else if (body.type === "text") {
    if (settled) return;
    const key = messageKey(e.commandId, body.messageId);
    view.messages[key] = {
      commandId: e.commandId,
      text: body.text,
      stable: true,
    };
    order(view, "message", key, e.sequence);
  } else if (body.type === "tool_proposal") {
    if (settled) return;
    const key = messageKey(e.commandId, body.proposalId);
    view.tools[key] = {
      commandId: e.commandId,
      proposalId: body.proposalId,
      name: body.name,
      arguments: body.arguments,
      status: "pending",
    };
    order(view, "tool", key, e.sequence);
  } else if (body.type === "tool_result") {
    if (settled) return;
    const tool = view.tools[messageKey(e.commandId, body.proposalId)];
    if (!tool) throw new ClientError("resync_required");
    tool.status = body.disposition === "returned" ? "completed" : "failed";
    tool.result = { disposition: body.disposition, text: body.text };
  } else if (body.type === "interaction") {
    if (body.status === "pending") {
      if (
        settled ||
        !command.dispatch ||
        command.dispatch.attemptId !== e.attemptId
      )
        throw new ClientError("resync_required");
      view.interactions[body.interactionId] = {
        commandId: e.commandId,
        ...(command.dispatch.nativeRunId
          ? { nativeRunId: command.dispatch.nativeRunId }
          : {}),
        status: body.status,
        request: body.request,
        expiresAtMs: body.expiresAtMs,
        callbackLifetime: body.callbackLifetime,
        generation: e.generation,
      };
      order(view, "interaction", body.interactionId, e.sequence);
    } else {
      const interaction = view.interactions[body.interactionId];
      if (!interaction) throw new ClientError("resync_required");
      interaction.status = body.status;
      if (body.status === "answered")
        interaction.responseCommandId = body.responseCommandId;
    }
  } else if (body.type === "surface") {
    const surface = validateSurface(body.surface),
      prior = view.surfaces[surface.surfaceInstanceId];
    if (
      prior &&
      (prior.status !== "active" || surface.revision !== prior.revision + 1)
    )
      throw new ClientError("resync_required");
    view.surfaces[surface.surfaceInstanceId] = surface;
    order(view, "surface", surface.surfaceInstanceId, e.sequence);
    if (
      surface.status !== "active" &&
      view.interactions[surface.interactionId]?.status === "pending"
    )
      view.interactions[surface.interactionId].status = "unavailable";
  }
}
/** Scratch view is published only after every page passes the same-watermark check. */
export function restoreSnapshot(
  view: SessionView,
  pages: SnapshotPage[],
): void {
  for (const page of pages) for (const e of page.events) event(view, e);
  if (pages.length) view.sessionStatus = pages[0].session.status;
  for (const page of pages) {
    for (const c of page.commands)
      view.commands[c.command.commandId] = {
        ...view.commands[c.command.commandId],
        command: structuredClone(c.command),
        state: c.state,
        ...(c.dispatch ? { dispatch: structuredClone(c.dispatch) } : {}),
        ...(c.outcome ? { outcome: c.outcome } : {}),
        ...(c.failure ? { failure: c.failure } : {}),
        ...(c.acknowledgement ? { acknowledgement: c.acknowledgement } : {}),
        ...(c.cancelledBy ? { cancelledBy: c.cancelledBy } : {}),
      };
    for (const interaction of page.interactions)
      view.interactions[interaction.interactionId] =
        projectInteraction(interaction);
    for (const surface of page.surfaces)
      view.surfaces[surface.surfaceInstanceId] = validateSurface(surface);
  }
}
/** Single product reducer. Standard ACP notifications and receipts do not write this state. */
export function applyUpdate(view: SessionView, item: Subscription): void {
  if (item.type === "resync_required") {
    view.connection = "resync_required";
    return;
  }
  if (view.connection === "resync_required") return;
  if (item.type === "delta") {
    if (
      item.generation !== view.generation ||
      !view.commands[item.commandId] ||
      ["terminal", "invalidated", "acknowledged", "cancelled"].includes(
        view.commands[item.commandId]?.state ?? "",
      )
    )
      return;
    const key = messageKey(item.commandId, item.messageId),
      prior = view.messages[key];
    if (prior?.stable) return;
    const text = (prior?.text ?? "") + item.text;
    const transient = Object.values(view.messages).filter(
      (m) => !m.stable && m !== prior,
    );
    if (
      transient.length >= 1024 ||
      transient.reduce(
        (n, m) => n + new TextEncoder().encode(m.text).byteLength,
        new TextEncoder().encode(text).byteLength,
      ) > accessLimits.maxTextBytes
    ) {
      view.connection = "resync_required";
      return;
    }
    order(view, "message", key, view.cursor + 0.5);
    view.messages[key] = {
      commandId: item.commandId,
      text,
      stable: false,
    };
    return;
  }
  const e = item.event,
    a = e.namespace,
    b = view.namespace;
  if (
    a.sessionId !== b.sessionId ||
    a.tenantId !== b.tenantId ||
    a.principalId !== b.principalId ||
    a.authorityId !== b.authorityId ||
    e.generation !== view.generation
  )
    throw new ClientError("resync_required");
  if (e.sequence <= view.cursor) return;
  if (e.sequence !== view.cursor + 1) {
    view.connection = "resync_required";
    return;
  }
  event(view, e);
  view.cursor = e.sequence;
}
