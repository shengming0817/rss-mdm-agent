import { ClientError } from "./errors.js";
import {
  validateSurface,
  accessLimits,
  type CommandState,
  type Event,
  type EventBody,
  type Interaction,
  type Outcome,
  type Session,
  type SnapshotPage,
  type Subscription,
  type SurfaceState,
} from "@rss-mdm-agent/ai-contract";

export type InteractionView = Pick<
  Interaction,
  | "status"
  | "request"
  | "expiresAtMs"
  | "callbackLifetime"
  | "responseCommandId"
  | "generation"
>;
const projectInteraction = (row: Interaction): InteractionView => ({
  status: row.status,
  request: row.request,
  expiresAtMs: row.expiresAtMs,
  callbackLifetime: row.callbackLifetime,
  generation: row.generation,
  ...(row.responseCommandId
    ? { responseCommandId: row.responseCommandId }
    : {}),
});
export interface SessionView {
  /** Identity of this attachment's snapshot, not a second live Host Session. */
  namespace: Session["namespace"];
  generation: string;
  cursor: number;
  connection: "attached" | "detached" | "resync_required";
  commands: Record<string, { state: CommandState; outcome?: Outcome }>;
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
        Extract<EventBody, { type: "tool_result" }>,
        "disposition" | "text"
      >;
    }
  >;
}
const messageKey = (command: string, message: string) =>
  `${command}/${message}`;
export function emptyView(session: Session, cursor: number): SessionView {
  return {
    namespace: structuredClone(session.namespace),
    generation: session.binding.generation,
    cursor,
    connection: "detached",
    commands: Object.create(null),
    messages: Object.create(null),
    interactions: Object.create(null),
    surfaces: Object.create(null),
    tools: Object.create(null),
  };
}
function event(view: SessionView, e: Event): void {
  const body = e.body;
  if (body.type === "terminal") {
    view.commands[e.commandId] = { state: "terminal", outcome: body.outcome };
    for (const [key, message] of Object.entries(view.messages))
      if (message.commandId === e.commandId && !message.stable)
        delete view.messages[key];
  } else if (body.type === "status") {
    if (view.commands[e.commandId]?.state !== "terminal")
      view.commands[e.commandId] = { state: body.state };
  } else if (body.type === "text") {
    view.messages[messageKey(e.commandId, body.messageId)] = {
      commandId: e.commandId,
      text: body.text,
      stable: true,
    };
  } else if (body.type === "tool_proposal") {
    view.tools[messageKey(e.commandId, body.proposalId)] = {
      commandId: e.commandId,
      proposalId: body.proposalId,
      name: body.name,
      arguments: body.arguments,
      status: "pending",
    };
  } else if (body.type === "tool_result") {
    const tool = view.tools[messageKey(e.commandId, body.proposalId)];
    if (!tool) throw new ClientError("resync_required");
    tool.status = body.disposition === "returned" ? "completed" : "failed";
    tool.result = { disposition: body.disposition, text: body.text };
  } else if (body.type === "interaction") {
    if (body.status === "pending")
      view.interactions[body.interactionId] = {
        status: body.status,
        request: body.request,
        expiresAtMs: body.expiresAtMs,
        callbackLifetime: body.callbackLifetime,
        generation: e.generation,
      };
    else {
      const interaction = view.interactions[body.interactionId];
      if (!interaction) throw new ClientError("resync_required");
      interaction.status = body.status;
      if (body.status === "answered")
        interaction.responseCommandId = body.responseCommandId;
    }
  } else if (body.type === "surface") {
    const surface = validateSurface(body.surface);
    const prior = view.surfaces[surface.surfaceInstanceId];
    if (
      prior &&
      (prior.status === "deleted" || surface.revision !== prior.revision + 1)
    )
      throw new ClientError("resync_required");
    view.surfaces[surface.surfaceInstanceId] = surface;
    if (
      surface.status === "deleted" &&
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
  for (const page of pages) {
    for (const c of page.commands)
      view.commands[c.command.commandId] = {
        state: c.state,
        ...(c.outcome ? { outcome: c.outcome } : {}),
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
      view.commands[item.commandId]?.state === "terminal"
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
