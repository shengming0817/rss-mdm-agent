import {
  client,
  type ClientConnection,
  type RequestPermissionRequest,
  type RequestPermissionResponse,
  type Stream,
} from "@agentclientprotocol/sdk";
import {
  accessLimits,
  boundedStream,
  boundedJson,
  decode,
  extension,
  interactionCatalog,
  parseNegotiation,
  type AccessUpdate,
  type ActionRequest,
  type Command,
  type Negotiation,
  type PageQuery,
  type Receipt,
  type Session,
  type SessionPage,
  type SnapshotPage,
  type WireRecord,
} from "@rss-mdm-agent/ai-contract";
import {
  appendSnapshot,
  applyUpdate,
  emptyView,
  type SessionView,
} from "./projection.js";

export interface ClientOptions {
  a2ui?: boolean;
  requestPermission?: (
    request: RequestPermissionRequest,
    signal: AbortSignal,
  ) => Promise<RequestPermissionResponse>;
}
const parse = <K extends WireRecord["kind"]>(
  input: unknown,
  kind: K,
): Extract<WireRecord, { kind: K }> => {
  const row = decode(boundedJson(input, accessLimits), accessLimits);
  if (row.kind !== kind) throw new Error("unexpected response kind");
  return row as Extract<WireRecord, { kind: K }>;
};
/** Browser-safe ACP client with one recoverable product projection per session. */
export class RuntimeClient {
  readonly connection: ClientConnection;
  private selected?: Negotiation;
  private views = new Map<string, SessionView>();
  private attachments = new Map<string, string>();
  private observers = new Set<(view: SessionView) => void>();
  constructor(
    stream: Stream,
    private options: ClientOptions = {},
  ) {
    const app = client({ name: "rss-mdm-agent-ai-client" });
    app.onRequest(
      "session/request_permission",
      ({ params, signal }) =>
        options.requestPermission?.(params, signal) ?? {
          outcome: { outcome: "cancelled" },
        },
    );
    app.onNotification("session/update", () => {});
    app.onNotification(
      extension.update,
      { parse: (input: unknown) => parse(input, "accessUpdate") },
      ({ params }) => this.update(params),
    );
    this.connection = app.connect(boundedStream(stream));
    void this.connection.closed.then(() => {
      this.attachments.clear();
      for (const view of this.views.values()) {
        view.connection = "detached";
        this.publish(view);
      }
    });
  }
  async initialize(): Promise<Negotiation> {
    const offered: Negotiation = {
      contractVersion: 2,
      acp: 1,
      durableReceipts: true,
      cursorAttach: true,
      ...(this.options.a2ui === false ? {} : { a2ui: interactionCatalog }),
    };
    const response = await this.connection.agent.request("initialize", {
      protocolVersion: 1,
      clientCapabilities: { _meta: { [extension.capability]: offered } },
      clientInfo: { name: "rss-mdm-agent-ai-client", version: "0.1.0" },
    });
    const selected = parseNegotiation(
      response.agentCapabilities?._meta?.[extension.capability],
      accessLimits,
    );
    if (
      response.protocolVersion !== 1 ||
      selected?.contractVersion !== 2 ||
      !selected.cursorAttach
    )
      throw new Error("runtime extension unavailable");
    if (
      selected.a2ui &&
      (selected.a2ui.version !== interactionCatalog.version ||
        selected.a2ui.catalogId !== interactionCatalog.catalogId ||
        selected.a2ui.catalogVersion !== interactionCatalog.catalogVersion)
    )
      throw new Error("A2UI catalog unavailable");
    this.selected = selected;
    return structuredClone(selected);
  }
  private ready(): void {
    if (!this.selected) throw new Error("initialize required");
  }
  async createSession(): Promise<SessionView> {
    this.ready();
    const result = await this.connection.agent.request("session/new", {
      cwd: "/",
      mcpServers: [],
    });
    return this.restore(result.sessionId);
  }
  async listSessions(query: PageQuery = { limit: 64 }): Promise<SessionPage> {
    this.ready();
    return parse(
      await this.connection.agent.request(extension.list, {
        schemaVersion: 2,
        kind: "listRequest",
        query,
      }),
      "sessionPage",
    );
  }
  async restore(sessionId: string, pageLimit = 64): Promise<SessionView> {
    this.ready();
    await this.detach(sessionId);
    const attachmentId = crypto.randomUUID();
    this.attachments.set(sessionId, attachmentId);
    let continuation: string | undefined,
      first: SnapshotPage | undefined,
      view: SessionView | undefined,
      index = 0,
      eventCursor = 0;
    try {
      do {
        if (index >= 4096) throw new Error("snapshot page budget");
        const page = parse(
          await this.connection.agent.request(extension.snapshot, {
            schemaVersion: 2,
            kind: "snapshotRequest",
            sessionId,
            query: {
              limit: pageLimit,
              ...(continuation ? { continuation } : {}),
            },
          }),
          "snapshotPage",
        );
        if (this.attachments.get(sessionId) !== attachmentId)
          throw new Error("restore superseded");
        first ??= page;
        if (
          page.pageIndex !== index++ ||
          page.snapshotId !== first.snapshotId ||
          page.cursor !== first.cursor ||
          JSON.stringify(page.session) !== JSON.stringify(first.session) ||
          page.session.namespace.sessionId !== sessionId
        )
          throw new Error("snapshot watermark mismatch");
        view ??= emptyView(page.session, page.cursor);
        for (const e of page.events) {
          if (e.sequence !== eventCursor + 1 || e.sequence > page.cursor)
            throw new Error("snapshot event order");
          eventCursor = e.sequence;
        }
        appendSnapshot(view, page);
        continuation = page.next;
      } while (continuation);
      if (!view || eventCursor !== view.cursor)
        throw new Error("snapshot history incomplete");
      this.views.set(sessionId, view);
      view.connection = "attached";
      await this.connection.agent.request(extension.attach, {
        schemaVersion: 2,
        kind: "attachRequest",
        sessionId,
        attachmentId,
        after: view.cursor,
      });
      this.publish(view);
      return this.getSession(sessionId)!;
    } catch (error) {
      if (this.attachments.get(sessionId) === attachmentId) {
        this.attachments.delete(sessionId);
        const prior = this.views.get(sessionId);
        if (prior) {
          prior.connection = "resync_required";
          this.publish(prior);
        }
      }
      throw error;
    }
  }
  async detach(sessionId: string): Promise<void> {
    const attachmentId = this.attachments.get(sessionId);
    this.attachments.delete(sessionId);
    const view = this.views.get(sessionId);
    if (view) {
      view.connection = "detached";
      this.publish(view);
    }
    if (attachmentId && !this.connection.signal.aborted)
      await this.connection.agent.request(extension.detach, {
        schemaVersion: 2,
        kind: "detachRequest",
        sessionId,
        attachmentId,
      });
  }
  async submit(command: Command): Promise<Receipt> {
    this.ready();
    return parse(
      await this.connection.agent.request(
        extension.submit,
        parse(command, "command"),
      ),
      "receipt",
    );
  }
  async action(request: ActionRequest): Promise<Receipt> {
    this.ready();
    if (!this.selected?.a2ui) throw new Error("A2UI not negotiated");
    return parse(
      await this.connection.agent.request(
        extension.action,
        parse(request, "actionRequest"),
      ),
      "receipt",
    );
  }
  async resume(sessionId: string): Promise<Session> {
    this.ready();
    return parse(
      await this.connection.agent.request(extension.resume, {
        schemaVersion: 2,
        kind: "resumeRequest",
        sessionId,
      }),
      "session",
    );
  }
  getSession(sessionId: string): SessionView | undefined {
    const view = this.views.get(sessionId);
    return view && structuredClone(view);
  }
  observe(listener: (view: SessionView) => void): () => void {
    this.observers.add(listener);
    return () => {
      this.observers.delete(listener);
    };
  }
  close(): void {
    this.connection.close();
  }
  private publish(view: SessionView): void {
    for (const observer of this.observers) observer(structuredClone(view));
  }
  private update(update: AccessUpdate): void {
    if (this.attachments.get(update.sessionId) !== update.attachmentId) return;
    const view = this.views.get(update.sessionId);
    if (!view) return;
    try {
      applyUpdate(view, update.update);
    } catch {
      view.connection = "resync_required";
    }
    this.publish(view);
  }
}
