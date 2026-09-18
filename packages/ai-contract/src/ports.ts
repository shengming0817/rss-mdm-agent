import type {
  Binding,
  Capabilities,
  Command,
  CommandRecord,
  Counter,
  Delivery,
  Event,
  Failure,
  Id,
  Interaction,
  Namespace,
  Receipt,
  Session,
  SurfaceBinding,
  ConfigRef,
} from "./wire.js";
/** Supplied by authenticated ingress, never decoded from model/tool/action content.
 * This port does not authenticate its caller; the composition root owns that proof. */
export interface Caller {
  readonly tenantId: Id;
  readonly principalId: Id;
  readonly authorityId: Id;
}
export interface Budget {
  readonly timeoutMs: number;
  readonly signal: AbortSignal;
  readonly maxSnapshotRecords?: number;
}
export interface Retention {
  readonly retryWindowMs: number;
  readonly receiptWindowMs: number;
}
export interface Clock {
  now(): number;
}
export type Result<T> = { ok: true; value: T } | { ok: false; error: Failure };
/** Claims alone cannot open this profile. A trusted adapter verifier provides exact evidence. */
export interface ControlledToolEvidence {
  readonly binding: Binding;
  readonly platform: string;
  readonly verificationRef: Id;
}
export interface Negotiation {
  readonly contractVersion: 2;
  readonly acp: 1;
  readonly a2ui?: {
    readonly version: "v0.9.1";
    readonly catalogId: Id;
    readonly catalogVersion: Id;
  };
  readonly durableReceipts: boolean;
  readonly cursorAttach: boolean;
}
export interface SessionOptions {
  readonly provider: Id;
  readonly config: ConfigRef;
  readonly accountRef: Id;
  readonly profile: "conversation" | "controlled_tools";
}
/** Only a proposal/result bridge; it cannot issue execution permits or approve a plan. */
export interface ToolEndpoint {
  propose(
    proposal: { name: Id; arguments: Record<string, unknown> },
    budget: Budget,
  ): Promise<
    Result<{
      disposition: "returned" | "rejected" | "unavailable";
      text: string;
    }>
  >;
}
export interface ProviderConfiguration {
  readonly config: ConfigRef;
  readonly accountRef: Id;
  readonly workingDirectory: string;
  readonly permissions: "tools_disabled" | "host_mediated";
  readonly tools?: ToolEndpoint;
}
export type ProviderObservation =
  | { type: "event"; binding: Binding; commandId: Id; body: Event["body"] }
  | {
      type: "delta";
      binding: Binding;
      commandId: Id;
      messageId: Id;
      text: string;
    };
export type Submission =
  | { certainty: "submitted"; binding: Binding }
  | { certainty: "not_sent"; error: Failure }
  | { certainty: "unknown"; correlationId: Id };
/** A01 contract only: each adapter owns its SDK, process and native context. */
export interface ProviderAgentPort {
  initialize(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<Capabilities>>;
  createSession(budget: Budget): Promise<Result<Binding>>;
  submit(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Submission>;
  cancel(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<"request_only" | "already_terminal" | "unsupported">>;
  respond(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<void>>;
  observe(binding: Binding, budget: Budget): AsyncIterable<ProviderObservation>;
  reconcile(
    binding: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<
    Result<{
      status: "running" | "terminal" | "not_submitted" | "unknown";
      binding: Binding;
      outcome?: import("./wire.js").Outcome;
    }>
  >;
  resume?(binding: Binding, budget: Budget): Promise<Result<Binding>>;
  verifyControlledTools?(
    binding: Binding,
    budget: Budget,
  ): Promise<Result<ControlledToolEvidence>>;
  close(budget: Budget): Promise<Result<{ processStopped: boolean }>>;
}
export interface Snapshot {
  readonly session: Session;
  readonly cursor: Counter;
  readonly events: readonly Event[];
  readonly commands: readonly CommandRecord[];
  readonly interactions: readonly Interaction[];
  readonly surfaces: readonly SurfaceBinding[];
}
export type Subscription =
  | { type: "event"; event: Event }
  | { type: "delta"; commandId: Id; generation: Id; text: string }
  | { type: "resync_required" };
/** Acceptance is durable only when backed by a real store; testing exports explicitly simulate it. */
export interface HostPort {
  negotiate(offered: Negotiation): Result<Negotiation>;
  createSession(
    caller: Caller,
    options: SessionOptions,
    budget: Budget,
  ): Promise<Result<Session>>;
  submit(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>>;
  cancel(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>>;
  respond(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>>;
  snapshot(
    caller: Caller,
    sessionId: Id,
    budget: Budget,
  ): Promise<Result<Snapshot>>;
  subscribe(
    caller: Caller,
    sessionId: Id,
    after: Counter,
    budget: Budget,
  ): AsyncIterable<Subscription>;
}
/** Atomic batch, checked against both revision and generation. No external await inside a transaction. */
export interface SessionCommit {
  readonly namespace: Namespace;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly session: Session;
  readonly commands: readonly CommandRecord[];
  readonly events: readonly Event[];
  readonly interactions: readonly Interaction[];
  readonly deliveries: readonly Delivery[];
  readonly surfaces: readonly SurfaceBinding[];
}
export interface AcceptCommand {
  readonly namespace: Namespace;
  readonly command: Command;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly nowMs: Counter;
  readonly retention: Retention;
  readonly event: Event;
}
export interface Page<T> {
  readonly items: readonly T[];
  readonly next?: Id;
}
/** One logical session coordinator; no distributed worker/lease promise. */
export interface SessionStore {
  create(session: Session): Promise<Result<void>>;
  session(namespace: Namespace): Promise<Result<Session>>;
  accept(input: AcceptCommand): Promise<Result<Receipt>>;
  command(namespace: Namespace, commandId: Id): Promise<Result<CommandRecord>>;
  commit(batch: SessionCommit): Promise<Result<void>>;
  snapshot(namespace: Namespace, limit: number): Promise<Result<Snapshot>>;
  events(
    namespace: Namespace,
    after: Counter,
    limit: number,
  ): Promise<Result<readonly Event[]>>;
  recovery(limit: number, after?: Id): Promise<Page<CommandRecord>>;
  deliveries(
    limit: number,
    nowMs: Counter,
    after?: Id,
  ): Promise<Page<Delivery>>;
  retire(
    namespace: Namespace,
    expectedRevision: Counter,
    expectedGeneration: Id,
  ): Promise<Result<void>>;
  /** Delete retired sessions only after all receipts expire and delivery is settled.
   * Missing/retired namespaces never implicitly recreate a session. */
  pruneRetired(nowMs: Counter): Promise<number>;
}
