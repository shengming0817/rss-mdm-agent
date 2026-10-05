import type {
  CommandRecord,
  Delivery,
  Event,
  Id,
  Interaction,
  Session,
  SurfaceState,
} from "./wire.js";
import type { SessionWorkspace } from "./transitions.js";

/** A transaction row lookup is deliberately not enumerable; scoped queries own enumeration. */
export interface SessionRows<K, V> {
  get(key: K): V | undefined;
  has(key: K): boolean;
  set(key: K, value: V): unknown;
}
/** Explicit materialization is valid only for the memory adapter and full cold snapshots. */
export function memoryRows<K, V>(rows: SessionRows<K, V>): Map<K, V> {
  if (!(rows instanceof Map))
    throw new TypeError("transaction rows require a scoped read view");
  return rows;
}
/** Synchronous, read-only transaction queries. The reducer owns every business predicate. */
export interface SessionQueries {
  command(id: Id): CommandRecord | undefined;
  event(id: Id): Event | undefined;
  interaction(id: Id): Interaction | undefined;
  surface(id: Id): SurfaceState | undefined;
  delivery(id: Id): Delivery | undefined;
  hasEvent(id: Id): boolean;
  hasAttempt(id: Id): boolean;
  commandsInStates(
    states: readonly CommandRecord["state"][],
  ): readonly CommandRecord[];
  commandsForNativeTurn(
    session: Id | undefined,
    thread: Id | undefined,
    run?: Id,
    request?: Id,
  ): readonly CommandRecord[];
  interactionForCallback(generation: Id, callback: Id): Interaction | undefined;
  interactionsForCommand(command?: Id): readonly Interaction[];
  surfacesForInteraction(interaction?: Id): readonly SurfaceState[];
  deliveriesInStates(
    states: readonly Delivery["status"][],
  ): readonly Delivery[];
  hasNotSubmittedReconciliation(command: Id): boolean;
}
import type { Result } from "./ports.js";
import type { Failure } from "./wire.js";
export type SessionReadView = {
  [K in keyof SessionQueries]: (
    ...args: Parameters<SessionQueries[K]>
  ) => Result<ReturnType<SessionQueries[K]>>;
};
export class TransactionReadFailure extends Error {
  constructor(readonly failure: Failure) {
    super(failure.code);
  }
}
export function resultView(
  queries: SessionQueries,
  safe: <T>(read: () => T) => Result<T>,
): SessionReadView {
  return {
    command: (id) => safe(() => queries.command(id)),
    event: (id) => safe(() => queries.event(id)),
    interaction: (id) => safe(() => queries.interaction(id)),
    surface: (id) => safe(() => queries.surface(id)),
    delivery: (id) => safe(() => queries.delivery(id)),
    hasEvent: (id) => safe(() => queries.hasEvent(id)),
    hasAttempt: (id) => safe(() => queries.hasAttempt(id)),
    commandsInStates: (states) => safe(() => queries.commandsInStates(states)),
    commandsForNativeTurn: (session, thread, run, request) =>
      safe(() => queries.commandsForNativeTurn(session, thread, run, request)),
    interactionForCallback: (generation, callback) =>
      safe(() => queries.interactionForCallback(generation, callback)),
    interactionsForCommand: (command) =>
      safe(() => queries.interactionsForCommand(command)),
    surfacesForInteraction: (interaction) =>
      safe(() => queries.surfacesForInteraction(interaction)),
    deliveriesInStates: (states) =>
      safe(() => queries.deliveriesInStates(states)),
    hasNotSubmittedReconciliation: (command) =>
      safe(() => queries.hasNotSubmittedReconciliation(command)),
  };
}
export function readResult<T>(value: Result<T>): T {
  if (!value.ok) throw new TransactionReadFailure(value.error);
  return value.value;
}
function queryView(view: SessionReadView): SessionQueries {
  return {
    command: (id) => readResult(view.command(id)),
    event: (id) => readResult(view.event(id)),
    interaction: (id) => readResult(view.interaction(id)),
    surface: (id) => readResult(view.surface(id)),
    delivery: (id) => readResult(view.delivery(id)),
    hasEvent: (id) => readResult(view.hasEvent(id)),
    hasAttempt: (id) => readResult(view.hasAttempt(id)),
    commandsInStates: (states) => readResult(view.commandsInStates(states)),
    commandsForNativeTurn: (session, thread, run, request) =>
      readResult(view.commandsForNativeTurn(session, thread, run, request)),
    interactionForCallback: (generation, callback) =>
      readResult(view.interactionForCallback(generation, callback)),
    interactionsForCommand: (command) =>
      readResult(view.interactionsForCommand(command)),
    surfacesForInteraction: (interaction) =>
      readResult(view.surfacesForInteraction(interaction)),
    deliveriesInStates: (states) => readResult(view.deliveriesInStates(states)),
    hasNotSubmittedReconciliation: (command) =>
      readResult(view.hasNotSubmittedReconciliation(command)),
  };
}
export interface SessionDelta {
  session: Session;
  generations: readonly Id[];
  commands: readonly CommandRecord[];
  events: readonly Event[];
  interactions: readonly Interaction[];
  surfaces: readonly SurfaceState[];
  deliveries: readonly Delivery[];
}
class Overlay<K, V> extends Map<K, V> {
  constructor(readonly base: SessionRows<K, V>) {
    super();
  }
  override get(key: K): V | undefined {
    return super.has(key) ? super.get(key) : this.base.get(key);
  }
  override has(key: K): boolean {
    return super.has(key) || this.base.has(key);
  }
  writes(): Map<K, V> {
    return new Map([
      ...(this.base instanceof Overlay ? this.base.writes() : []),
      ...super.entries(),
    ]);
  }
  override *entries(): MapIterator<[K, V]> {
    const rows = new Map(memoryRows(this.base));
    for (const [k, v] of super.entries()) rows.set(k, v);
    yield* rows;
  }
  override [Symbol.iterator](): MapIterator<[K, V]> {
    return this.entries();
  }
  override *values(): MapIterator<V> {
    for (const [, v] of this.entries()) yield v;
  }
}
/** Fork only the small session head and pending delta; source rows remain immutable. */
export function forkState(state: SessionWorkspace): SessionWorkspace {
  return {
    session: structuredClone(state.session),
    source: state.source,
    generations: new Set(state.generations),
    commands: new Overlay(state.commands),
    interactions: new Overlay(state.interactions),
    surfaces: new Overlay(state.surfaces),
    deliveries: new Overlay(state.deliveries),
    events: [...state.events],
  };
}
const writes = <K, V>(rows: SessionRows<K, V>): Map<K, V> =>
  rows instanceof Overlay ? rows.writes() : new Map();
const merge = <T>(
  base: readonly T[],
  changed: Map<Id, T>,
  key: (row: T) => Id,
  predicate: (row: T) => boolean,
): T[] => {
  const rows = new Map(base.map((row) => [key(row), row]));
  for (const [id, row] of changed) {
    if (predicate(row)) rows.set(id, row);
    else rows.delete(id);
  }
  return [...rows.values()];
};
export function transactionDelta(
  before: SessionWorkspace,
  after: SessionWorkspace,
): SessionDelta {
  return {
    session: after.session,
    generations: [...after.generations].filter(
      (id) => !before.generations.has(id),
    ),
    commands: [...writes(after.commands).values()],
    events: after.events.slice(before.events.length),
    interactions: [...writes(after.interactions).values()],
    surfaces: [...writes(after.surfaces).values()],
    deliveries: [...writes(after.deliveries).values()],
  };
}
/** Memory maps and SQL projections use these identical reducer-facing queries. */
export function readView(state: SessionWorkspace): SessionQueries {
  const source = state.source && queryView(state.source);
  const commands = (
    predicate: (c: CommandRecord) => boolean,
    base: readonly CommandRecord[],
  ) =>
    merge(base, writes(state.commands), (c) => c.command.commandId, predicate);
  const interactions = (
    predicate: (i: Interaction) => boolean,
    base: readonly Interaction[],
  ) =>
    merge(base, writes(state.interactions), (i) => i.interactionId, predicate);
  const surfaces = (
    predicate: (s: SurfaceState) => boolean,
    base: readonly SurfaceState[],
  ) =>
    merge(base, writes(state.surfaces), (s) => s.surfaceInstanceId, predicate);
  return {
    command: (id) => state.commands.get(id),
    interaction: (id) => state.interactions.get(id),
    surface: (id) => state.surfaces.get(id),
    delivery: (id) => state.deliveries.get(id),
    event: (id) =>
      state.events.find((e) => e.eventId === id) ?? source?.event(id),
    hasEvent: (id) =>
      state.events.some((e) => e.eventId === id) || !!source?.hasEvent(id),
    hasAttempt: (id) =>
      state.events.some((e) => e.attemptId === id) || !!source?.hasAttempt(id),
    commandsInStates: (states) => {
      const predicate = (c: CommandRecord) => states.includes(c.state);
      return commands(
        predicate,
        source?.commandsInStates(states) ??
          [...memoryRows(state.commands).values()].filter(predicate),
      );
    },
    commandsForNativeTurn: (session, thread, run, request) => {
      const predicate = (c: CommandRecord) =>
        !!c.dispatch &&
        c.dispatch.nativeSessionId === session &&
        c.dispatch.nativeThreadId === thread &&
        (run !== undefined
          ? c.dispatch.nativeRunId === run
          : c.dispatch.nativeRunId === run &&
            c.dispatch.nativeRequestId === request);
      return commands(
        predicate,
        source?.commandsForNativeTurn(session, thread, run, request) ??
          [...memoryRows(state.commands).values()].filter(predicate),
      );
    },
    interactionForCallback: (generation, callback) => {
      const predicate = (i: Interaction) =>
        i.generation === generation && i.nativeCallbackId === callback;
      const old = source?.interactionForCallback(generation, callback);
      return interactions(
        predicate,
        source
          ? old
            ? [old]
            : []
          : [...memoryRows(state.interactions).values()].filter(predicate),
      )[0];
    },
    interactionsForCommand: (command) => {
      const predicate = (i: Interaction) =>
        command === undefined || i.commandId === command;
      return interactions(
        predicate,
        source?.interactionsForCommand(command) ??
          [...memoryRows(state.interactions).values()].filter(predicate),
      );
    },
    surfacesForInteraction: (interaction) => {
      const predicate = (s: SurfaceState) =>
        interaction === undefined || s.interactionId === interaction;
      return surfaces(
        predicate,
        source?.surfacesForInteraction(interaction) ??
          [...memoryRows(state.surfaces).values()].filter(predicate),
      );
    },
    deliveriesInStates: (states) => {
      const predicate = (d: Delivery) => states.includes(d.status);
      return merge(
        source?.deliveriesInStates(states) ??
          [...memoryRows(state.deliveries).values()].filter(predicate),
        writes(state.deliveries),
        (d) => d.operationId,
        predicate,
      );
    },
    hasNotSubmittedReconciliation: (command) =>
      state.events.some(
        (e) =>
          e.commandId === command &&
          e.body.type === "reconciled" &&
          e.body.resolution === "not_submitted",
      ) || !!source?.hasNotSubmittedReconciliation(command),
  };
}

/** Materialization belongs only to the memory adapter; SQL consumes the explicit delta. */
export function memoryState(state: SessionWorkspace): SessionWorkspace {
  if (state.source) return state;
  return {
    ...state,
    commands: new Map(memoryRows(state.commands)),
    interactions: new Map(memoryRows(state.interactions)),
    surfaces: new Map(memoryRows(state.surfaces)),
    deliveries: new Map(memoryRows(state.deliveries)),
  };
}
