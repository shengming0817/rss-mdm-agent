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
  SurfaceState,
  SnapshotPage,
  PageQuery,
  SessionPage,
  Subscription,
  ConfigRef,
  Negotiation,
  DispatchAttempt,
  Outcome,
  Acknowledgement,
  Connection,
  UserPreferences,
  ConnectionPage,
  HistoryPreview,
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
}
export interface Retention {
  readonly retryWindowMs: number;
  readonly receiptWindowMs: number;
}
export interface Clock {
  now(): number;
}
export type Result<T> = { ok: true; value: T } | { ok: false; error: Failure };
/** Atomic provider result: capabilities describe this exact incarnation. */
export interface ProviderSessionBinding {
  readonly binding: Binding;
  readonly capabilities: Capabilities;
}
/** Trusted composition-owned platform verifier, never supplied by model/wire input. */
export interface ControlledToolVerifier {
  verify(
    session: ProviderSessionBinding,
    tools: ToolEndpoint,
    budget: Budget,
  ): Promise<
    Result<{
      platform: string;
      verificationRef: Id;
    }>
  >;
}
export interface SessionOptions {
  readonly connectionId?: Id;
}
export interface ConnectionOptions {
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
interface ProviderConfigurationBase {
  readonly namespace: Namespace;
  readonly provider: Id;
  readonly config: ConfigRef;
  readonly accountRef: Id;
  readonly workingDirectory: string;
}
/** Pure data crossing the private worker IPC boundary. */
export interface ProviderConfiguration extends ProviderConfigurationBase {
  readonly permissions: "tools_disabled" | "host_mediated";
}
/** Parent-only admission inputs. The verifier never crosses worker IPC. */
export interface ProviderAdmission {
  readonly tools: ToolEndpoint;
  readonly verifier: ControlledToolVerifier;
}
export interface MessageDelta {
  readonly commandId: Id;
  readonly messageId: Id;
  readonly text: string;
}
/** Callback metadata from a live provider. Host supplies namespace, command and
 * generation from the verified observation binding, then atomically stores the
 * pending Interaction and its matching event. Display recovery cannot revive a callback. */
export type ProviderInteraction = Pick<
  Interaction,
  | "category"
  | "interactionId"
  | "nativeCallbackId"
  | "expiresAtMs"
  | "callbackLifetime"
  | "request"
>;
export type ProviderEventBody = Extract<
  Event["body"],
  {
    type:
      | "text"
      | "terminal"
      | "tool_proposal"
      | "tool_result"
      | "error"
      | "cancel_dispatched"
      | "surface";
  }
>;
export type ProviderObservation = { readonly attemptId: Id } & (
  | { type: "submitted"; binding: Binding; commandId: Id }
  | { type: "running"; binding: Binding; commandId: Id }
  | {
      type: "acknowledged";
      binding: Binding;
      commandId: Id;
      acknowledgement: Acknowledgement;
    }
  | {
      type: "interaction_unavailable";
      binding: Binding;
      commandId: Id;
      interactionId: Id;
    }
  | {
      type: "event";
      binding: Binding;
      commandId: Id;
      body: ProviderEventBody;
    }
  | ({ type: "delta"; binding: Binding } & MessageDelta)
  | {
      type: "interaction";
      binding: Binding;
      commandId: Id;
      interaction: ProviderInteraction;
    }
);
export type Submission =
  | {
      certainty: "acknowledged";
      binding: Binding;
      acknowledgement: Acknowledgement;
    }
  | { certainty: "submitted"; binding: Binding }
  | { certainty: "not_sent"; error: Failure }
  | { certainty: "unknown"; correlationId: Id };
/** Each port instance owns one admitted session incarnation. Open/restore consume
 * it once; failure closes that instance. Close is idempotent, retryable and must
 * also clean up creation/resume work that settles after abort. */
export interface ProviderAgentPort {
  createSession(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>>;
  dispatch(
    binding: Binding,
    command: Command,
    attempt: DispatchAttempt,
    budget: Budget,
  ): Promise<Submission>;
  observe(binding: Binding, budget: Budget): AsyncIterable<ProviderObservation>;
  reconcile(
    binding: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<Result<Reconciliation>>;
  resume?(
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>>;
  close(budget: Budget): Promise<Result<{ processStopped: boolean }>>;
}
/** Close stops admission, ends subscriptions/workers, then closes provider and store.
 * It is idempotent; failed cleanup may be retried with a fresh budget. */
export interface Closeable {
  close(budget: Budget): Promise<Result<void>>;
}
/** Acceptance is durable only when backed by a real store; testing exports explicitly simulate it. */
export interface HostPort extends Closeable {
  connections(caller: Caller, budget: Budget): Promise<Result<ConnectionPage>>;
  saveConnection(
    caller: Caller,
    connection: Connection,
    expectedRevision: Counter | null,
    budget: Budget,
  ): Promise<Result<Connection>>;
  savePreferences(
    caller: Caller,
    preferences: UserPreferences,
    budget: Budget,
  ): Promise<Result<UserPreferences>>;
  selectConnection(
    caller: Caller,
    sessionId: Id,
    connectionId: Id,
    budget: Budget,
    freshContext?: boolean,
  ): Promise<Result<Session>>;
  previewHistory(
    caller: Caller,
    sessionId: Id,
    connectionId: Id,
    recent: Counter | undefined,
    budget: Budget,
  ): Promise<Result<HistoryPreview>>;
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
  surface(
    caller: Caller,
    sessionId: Id,
    instanceId: Id,
    budget: Budget,
  ): Promise<Result<SurfaceState>>;
  snapshotPage(
    caller: Caller,
    sessionId: Id,
    query: PageQuery,
    budget: Budget,
  ): Promise<Result<SnapshotPage>>;
  listSessions(
    caller: Caller,
    query: PageQuery,
    budget: Budget,
  ): Promise<Result<SessionPage>>;
  resume(
    caller: Caller,
    sessionId: Id,
    budget: Budget,
  ): Promise<Result<Session>>;
  subscribe(
    caller: Caller,
    sessionId: Id,
    after: Counter,
    budget: Budget,
  ): AsyncIterable<Subscription>;
}
/** Atomic batch, checked against both revision and generation. No external await inside a transaction. */
export interface SessionCommit {
  /** Verified provider reconciliation observations; consumed by the same transition rules. */
  readonly providerFacts?: readonly import("./session.js").VerifiedProviderFact[];
  /** Required for retry eligibility and local expiry transitions. */
  readonly nowMs?: Counter;
  readonly namespace: Namespace;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly session: Session;
  readonly commands: readonly CommandRecord[];
  readonly events: readonly Event[];
  readonly interactions: readonly Interaction[];
  readonly deliveries: readonly Delivery[];
  readonly surfaces: readonly SurfaceState[];
}
export interface AcceptCommand {
  readonly namespace: Namespace;
  readonly command: Command;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly nowMs: Counter;
  readonly retention: Retention;
  readonly eventId: Id;
}
/** Opaque process-independent continuation, 1–2048 characters; not a wire Id. */
export type StoreCursor = string;
export interface Page<T> {
  readonly items: readonly T[];
  readonly next?: StoreCursor;
}
/** One logical session coordinator; no distributed worker/lease promise. */
export interface SessionStore extends Closeable {
  connections(caller: Caller): Promise<Result<readonly Connection[]>>;
  connection(
    caller: Caller,
    id: Id,
    revision?: Counter,
  ): Promise<Result<Connection>>;
  saveConnection(
    caller: Caller,
    connection: Connection,
    expectedRevision: Counter | null,
  ): Promise<Result<Connection>>;
  preferences(caller: Caller): Promise<Result<UserPreferences>>;
  savePreferences(
    caller: Caller,
    preferences: UserPreferences,
  ): Promise<Result<UserPreferences>>;
  selectConnection(
    namespace: Namespace,
    connectionId: Id,
    expectedRevision: Counter,
    freshContext?: boolean,
  ): Promise<Result<Session>>;
  activateStage(input: StageActivation): Promise<Result<Session>>;
  create(session: Session): Promise<Result<void>>;
  session(namespace: Namespace): Promise<Result<Session>>;
  accept(input: AcceptCommand): Promise<Result<Receipt>>;
  surface(namespace: Namespace, instanceId: Id): Promise<Result<SurfaceState>>;
  command(namespace: Namespace, commandId: Id): Promise<Result<CommandRecord>>;
  /** Exact immutable outbox request, including settled deliveries; null means absent. */
  delivery(
    namespace: Namespace,
    operationId: Id,
  ): Promise<Result<{ delivery: Delivery; event: Event } | null>>;
  commit(batch: SessionCommit): Promise<Result<void>>;
  snapshotPage(
    namespace: Namespace,
    query: PageQuery,
  ): Promise<Result<SnapshotPage>>;
  listSessions(caller: Caller, query: PageQuery): Promise<Result<SessionPage>>;
  rebind(input: SessionRebind): Promise<Result<Session>>;
  recoverUnavailable(input: RecoveryUnavailable): Promise<Result<Session>>;
  events(
    namespace: Namespace,
    after: Counter,
    limit: number,
  ): Promise<Result<readonly Event[]>>;
  recovery(
    limit: number,
    after?: StoreCursor,
  ): Promise<Result<Page<CommandRecord>>>;
  deliveries(
    limit: number,
    nowMs: Counter,
    after?: StoreCursor,
  ): Promise<Result<Page<Delivery>>>;
  retire(
    namespace: Namespace,
    expectedRevision: Counter,
    expectedGeneration: Id,
  ): Promise<Result<void>>;
  /** Delete retired sessions only after all receipts expire and delivery is settled.
   * Missing/retired namespaces never implicitly recreate a session. */
  pruneRetired(nowMs: Counter): Promise<Result<number>>;
}

/** A fresh phase needs real provider admission; a wire record cannot mint it. */
export interface StageActivation {
  readonly namespace: Namespace;
  readonly expectedRevision: Counter;
  readonly configRevision: Counter;
  readonly credentialRevision: Counter;
  readonly opened: import("./session.js").VerifiedProviderSession;
}

export type { Subscription, Negotiation } from "./wire.js";
/** Bound to the original attempt and the current observer; unknown is not permission to send. */
export type Reconciliation = {
  readonly commandId: Id;
  readonly attemptId: Id;
  readonly binding: Binding;
} & (
  | { readonly status: "running" | "submitted"; readonly outcome?: never }
  | {
      readonly status: "unknown";
      readonly correlationId?: Id;
      readonly outcome?: never;
    }
  | { readonly status: "terminal"; readonly outcome: Outcome }
  | {
      readonly status: "not_submitted";
      readonly outcome?: never;
      readonly error?: Failure;
    }
  | {
      readonly status: "acknowledged";
      readonly acknowledgement: Acknowledgement;
      readonly outcome?: never;
    }
  | {
      readonly status: "observed";
      readonly observation: ProviderObservation;
      readonly outcome?: never;
    }
);
/** A trusted resume result, never a wire-decoded capability assertion. */
export interface SessionRebind {
  readonly namespace: Namespace;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly restored: import("./session.js").VerifiedProviderSession;
  readonly eventId: Id;
}

export interface RecoveryUnavailable {
  readonly namespace: Namespace;
  readonly expectedRevision: Counter;
  readonly expectedGeneration: Id;
  readonly eventId: Id;
}

/** Provider-owned native operation on a fresh, Host-owned child incarnation. */
export interface ProviderForkRequest {
  readonly namespace: Namespace;
  readonly binding: Binding;
  readonly throughTurnId: Id;
}
export type ProviderForkResult =
  | {
      certainty: "created";
      value: ProviderSessionBinding;
      source: ProviderForkRequest;
    }
  | { certainty: "not_created" | "unknown"; error: Failure };
export interface ProviderForkPort {
  forkSession(
    request: ProviderForkRequest,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<ProviderForkResult>;
}
/** Closed metadata only. No native method names, identifiers, paths or payloads. */
export interface ProviderDiagnostic {
  readonly kind: "text_delta" | "item_completed" | "turn_completed" | "other";
  readonly dropped: number;
}
export interface ProviderDiagnosticsPort {
  observe(binding: Binding, budget: Budget): AsyncIterable<ProviderDiagnostic>;
}
/** Extensions cannot create, admit, persist or dispose another provider instance. */
export interface ProviderInstance {
  readonly agent: ProviderAgentPort;
  readonly extensions: { readonly fork?: ProviderForkPort };
  readonly diagnostics: ProviderDiagnosticsPort;
}
