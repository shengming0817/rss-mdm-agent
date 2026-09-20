import {
  activeStage,
  emptyPreferences,
  mergePreferences,
  type PreferencesPatch,
  connectionRevision,
  type Connection,
  type UserPreferences,
  type StageActivation,
} from "@rss-mdm-agent/ai-contract";
import {
  validLaunch,
  type WorkerLaunch,
  type WorkerLaunchFenceStore,
} from "@rss-mdm-agent/ai-host/launch-fence";
import {
  ReadViews,
  readSnapshotPage,
  readSessionPage,
} from "@rss-mdm-agent/ai-contract/read-views";
import { DatabaseSync, type SQLInputValue } from "node:sqlite";
import {
  closeSync,
  constants,
  lstatSync,
  mkdirSync,
  openSync,
  realpathSync,
  readFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, basename } from "node:path";
import {
  boundedJson,
  decode,
  ContractError,
  isId,
  type AcceptCommand,
  type Budget,
  type CommandRecord,
  type Counter,
  type Delivery,
  type Event,
  type Id,
  type Namespace,
  type Page,
  type Receipt,
  type Result,
  type Session,
  type SessionCommit,
  type SessionRebind,
  type RecoveryUnavailable,
  type SessionSuspension,
  type SessionStore,
  type StoreCursor,
  type SnapshotPage,
  type SessionPage,
  type PageQuery,
  type Caller,
  type SurfaceState,
  type WireRecord,
} from "@rss-mdm-agent/ai-contract";
import {
  acceptCommand,
  selectConnection,
  activateStage,
  commitSession,
  isDeliveryCommit,
  createState,
  rebindSession,
  recoverUnavailable,
  suspendSession,
  retireSession,
  defaultLimits,
  namespaceKey,
  ok,
  fail,
  type SessionState,
} from "@rss-mdm-agent/ai-contract/transitions";
import {
  initialize,
  validateExisting,
  SchemaError,
  scope,
  whereScope,
} from "./schema.js";

/** All numeric bounds are positive safe integers. Full ranges are documented
 * in the package README capacity table. */
export interface StoreOptions {
  /** Absolute file path in a private local directory. */
  readonly path: string;
  /** create refuses existing files; open refuses missing/uninitialized databases. */
  readonly mode: "create" | "open";
  readonly busyTimeoutMs?: number;
  readonly maxDatabaseBytes?: number;
  readonly maxSessionRecords?: number;
  readonly maxSessionBytes?: number;
  readonly maxBatchRecords?: number;
  readonly maxQueryBytes?: number;
}
interface Bounds {
  busyTimeoutMs: number;
  maxDatabaseBytes: number;
  maxSessionRecords: number;
  maxSessionBytes: number;
  maxBatchRecords: number;
  maxQueryBytes: number;
}
const defaults: Bounds = {
  busyTimeoutMs: 1000,
  maxDatabaseBytes: 256 * 1024 * 1024,
  maxSessionRecords: 10000,
  maxSessionBytes: 16 * 1024 * 1024,
  maxBatchRecords: 1024,
  maxQueryBytes: 4 * 1024 * 1024,
};
const tables = [
  "commands",
  "events",
  "interactions",
  "surfaces",
  "deliveries",
] as const;
type Table = (typeof tables)[number];
const counter = (v: unknown): v is number =>
  Number.isSafeInteger(v) && Number(v) >= 0;
const nsValues = (n: Namespace): string[] => {
  namespaceKey(n);
  return [n.tenantId, n.principalId, n.authorityId, n.sessionId];
};
function errorResult(error: unknown): Result<never> {
  if (error instanceof SchemaError) return fail(error.code);
  if (error instanceof ContractError)
    return fail(error.code === "limit" ? "limit_exceeded" : "invalid_input");
  if (error instanceof InputError) return fail(error.code);
  if (typeof error === "object" && error !== null) {
    const native = error as { errcode?: unknown; code?: unknown };
    if (
      typeof native.errcode === "number" &&
      Number.isSafeInteger(native.errcode)
    ) {
      // SQLite extended result codes preserve the primary reason in the low byte.
      switch (native.errcode & 255) {
        case 5:
        case 6:
          return fail("unavailable", "same_command");
        case 11:
        case 26:
          return fail("storage_corrupt");
        case 13:
          return fail("limit_exceeded");
        case 3:
        case 8:
        case 23:
          return fail("permission_denied");
      }
    }
    if (native.code === "EACCES" || native.code === "EPERM")
      return fail("permission_denied");
    if (
      ["ENOENT", "EEXIST", "ENOTDIR", "EISDIR", "EINVAL"].includes(
        String(native.code),
      )
    )
      return fail("invalid_input");
  }
  // Unknown I/O failures may have ambiguous effects; never encourage blind retry.
  return fail("unavailable");
}
class InputError extends Error {
  constructor(
    readonly code:
      | "invalid_input"
      | "limit_exceeded"
      | "session_gone"
      | "stale_binding"
      | "content_conflict",
  ) {
    super(`AI SQLite: ${code}`);
  }
}
function bounds(options: StoreOptions): Bounds {
  const result = { ...defaults };
  for (const key of Object.keys(defaults) as (keyof Bounds)[]) {
    const value = options[key] ?? defaults[key];
    const maximum =
      key === "busyTimeoutMs"
        ? 10000
        : key === "maxSessionRecords" || key === "maxBatchRecords"
          ? 100000
          : 1024 * 1024 * 1024;
    if (!Number.isSafeInteger(value) || value < 1 || value > maximum)
      throw new InputError("invalid_input");
    result[key] = value;
  }
  if (result.maxDatabaseBytes < 65536) throw new InputError("invalid_input");
  return result;
}
function privatePath(options: StoreOptions): string {
  if (!isAbsolute(options.path) || !["create", "open"].includes(options.mode))
    throw new InputError("invalid_input");
  const parent = dirname(options.path);
  if (options.mode === "create")
    mkdirSync(parent, { recursive: true, mode: 0o700 });
  const directory = realpathSync(parent),
    stat = lstatSync(directory);
  if (
    !stat.isDirectory() ||
    (process.platform !== "win32" && (stat.mode & 0o077) !== 0)
  )
    throw new InputError("invalid_input");
  const path = join(directory, basename(options.path));
  if (options.mode === "create")
    closeSync(
      openSync(
        path,
        constants.O_CREAT | constants.O_EXCL | constants.O_RDWR,
        0o600,
      ),
    );
  const file = lstatSync(path);
  if (
    !file.isFile() ||
    file.isSymbolicLink() ||
    file.nlink !== 1 ||
    (process.platform !== "win32" && (file.mode & 0o077) !== 0)
  )
    throw new InputError("invalid_input");
  return path;
}

function pageLimit(db: DatabaseSync, maxBytes: number): number {
  const size = Number(db.prepare("PRAGMA page_size").get()!.page_size);
  const pages = Math.floor(maxBytes / size);
  if (Number(db.prepare("PRAGMA page_count").get()!.page_count) > pages)
    throw new InputError("limit_exceeded");
  return pages;
}

/** One Host owns the whole database for this connection's lifetime. There is no
 * lease, background worker, side-effect execution or asynchronous transaction hook. */
export function openSqliteStore(
  options: StoreOptions,
): Result<SessionStore & WorkerLaunchFenceStore> {
  let db: DatabaseSync | undefined;
  try {
    if (
      process.versions.node !==
      JSON.parse(
        readFileSync(new URL("../package.json", import.meta.url), "utf8"),
      ).engines.node
    )
      return fail("unsupported_version");
    const limits = bounds(options),
      path = privatePath(options);
    if (options.mode === "open") {
      const probe = new DatabaseSync(path, {
        readOnly: true,
        timeout: limits.busyTimeoutMs,
        enableForeignKeyConstraints: true,
        enableDoubleQuotedStringLiterals: false,
        allowExtension: false,
      });
      try {
        probe.exec("PRAGMA trusted_schema=OFF");
        pageLimit(probe, limits.maxDatabaseBytes);
        validateExisting(probe);
      } finally {
        probe.close();
      }
    }
    db = new DatabaseSync(path, {
      timeout: limits.busyTimeoutMs,
      enableForeignKeyConstraints: true,
      enableDoubleQuotedStringLiterals: false,
      allowExtension: false,
    });
    db.exec(
      "PRAGMA locking_mode=EXCLUSIVE; PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL",
    );
    const pages = pageLimit(db, limits.maxDatabaseBytes);
    // Recheck under the writer lock before any persistent PRAGMA change.
    if (options.mode === "open") initialize(db, false);
    if (db.prepare("PRAGMA journal_mode=WAL").get()!.journal_mode !== "wal")
      throw new SchemaError("unsupported_version");
    db.exec("PRAGMA wal_autocheckpoint=256");
    db.exec(`PRAGMA journal_size_limit=${limits.maxDatabaseBytes}`);
    db.exec(`PRAGMA max_page_count=${pages}`);
    if (
      db.prepare("SELECT sqlite_version() AS version").get()!.version !==
      "3.51.2"
    )
      throw new SchemaError("unsupported_version");
    if (options.mode === "create") initialize(db, true);
    return ok(new SqliteSessionStore(db, limits));
  } catch (error) {
    try {
      db?.close();
    } catch {
      /* preserve the value-free primary failure */
    }
    return errorResult(error);
  }
}
class SqliteSessionStore implements SessionStore, WorkerLaunchFenceStore {
  readonly #db: DatabaseSync;
  readonly #bounds: Bounds;
  readonly #owned = new Map<string, string>();
  readonly #views = new ReadViews({ now: () => Date.now() }, 30000);
  #closed = false;
  #closing = false;
  constructor(db: DatabaseSync, limits: Bounds) {
    this.#db = db;
    this.#bounds = limits;
  }
  #query<T>(action: () => Result<T>): Result<T> {
    if (this.#closing || this.#closed) return fail("unavailable");
    try {
      return action();
    } catch (error) {
      return errorResult(error);
    }
  }
  #transaction<T>(action: () => Result<T>): Result<T> {
    return this.#query(() => {
      this.#db.exec("BEGIN IMMEDIATE");
      try {
        const result = action();
        this.#db.exec(result.ok ? "COMMIT" : "ROLLBACK");
        return result;
      } catch (error) {
        try {
          if (this.#db.isTransaction) this.#db.exec("ROLLBACK");
        } catch {
          this.#closing = true;
          try {
            this.#db.close();
            this.#closed = true;
            this.#owned.clear();
            this.#views.clear();
          } catch {
            /* cleanup remains retryable through close */
          }
        }
        return errorResult(error);
      }
    });
  }
  #decode<T extends WireRecord>(json: unknown, kind: T["kind"]): T {
    if (typeof json !== "string") throw new SchemaError();
    try {
      const row = decode(json, defaultLimits);
      if (row.kind !== kind) throw new SchemaError();
      return row as T;
    } catch (error) {
      if (error instanceof ContractError && error.code === "limit") throw error;
      throw new SchemaError();
    }
  }
  #session(n: Namespace): Session {
    const row = this.#db
      .prepare(`SELECT json FROM sessions WHERE ${whereScope}`)
      .get(...nsValues(n));
    if (!row) throw new InputError("session_gone");
    const session = this.#decode<Session>(row.json, "session");
    if (namespaceKey(session.namespace) !== namespaceKey(n))
      throw new SchemaError();
    return session;
  }
  #state(n: Namespace): SessionState {
    const session = this.#session(n),
      params = nsValues(n);
    const totals = this.#db
      .prepare(
        tables
          .map(
            (t) =>
              `SELECT count(*) AS count,coalesce(sum(length(CAST(json AS BLOB))),0) AS bytes FROM ${t} WHERE ${whereScope}`,
          )
          .join(" UNION ALL ") +
          ` UNION ALL SELECT count(*) AS count,coalesce(sum(length(id)),0) AS bytes FROM generations WHERE ${whereScope}`,
      )
      .all(...tables.flatMap(() => params), ...params);
    if (
      totals.reduce((a, r) => a + Number(r.count), 1) >
        this.#bounds.maxSessionRecords ||
      totals.reduce(
        (a, r) => a + Number(r.bytes),
        Buffer.byteLength(JSON.stringify(session)),
      ) > this.#bounds.maxSessionBytes
    )
      throw new InputError("limit_exceeded");
    const load = <T extends WireRecord>(table: Table, kind: T["kind"]) =>
      this.#db
        .prepare(
          `SELECT json FROM ${table} WHERE ${whereScope}${table === "events" ? " ORDER BY sequence" : " ORDER BY id"}`,
        )
        .all(...params)
        .map((r) => this.#decode<T>(r.json, kind));
    const commands = load<CommandRecord>("commands", "commandRecord"),
      events = load<Event>("events", "event"),
      interactions = load<SnapshotPage["interactions"][number]>(
        "interactions",
        "interaction",
      ),
      surfaces = load<SurfaceState>("surfaces", "surface"),
      deliveries = load<Delivery>("deliveries", "delivery");
    if (
      events.length !== session.lastSequence ||
      events.some((e, i) => e.sequence !== i + 1)
    )
      throw new SchemaError();
    const generations = new Set(
      this.#db
        .prepare(`SELECT id FROM generations WHERE ${whereScope}`)
        .all(...params)
        .map((r) => String(r.id)),
    );
    if (
      session.currentStageId &&
      !generations.has(activeStage(session).binding.generation)
    )
      throw new SchemaError();
    return {
      session,
      generations,
      commands: new Map(commands.map((c) => [c.command.commandId, c])),
      events,
      interactions: new Map(interactions.map((i) => [i.interactionId, i])),
      surfaces: new Map(surfaces.map((s) => [s.surfaceInstanceId, s])),
      deliveries: new Map(deliveries.map((d) => [d.operationId, d])),
    };
  }
  #checkCapacity(state: SessionState): void {
    const rows = [
      state.session,
      ...state.commands.values(),
      ...state.events,
      ...state.interactions.values(),
      ...state.surfaces.values(),
      ...state.deliveries.values(),
    ];
    if (rows.length + state.generations.size > this.#bounds.maxSessionRecords)
      throw new InputError("limit_exceeded");
    let bytes = [...state.generations].reduce(
      (sum, generation) => sum + Buffer.byteLength(generation),
      0,
    );
    for (const row of rows) {
      bytes += Buffer.byteLength(boundedJson(row, defaultLimits));
      if (bytes > this.#bounds.maxSessionBytes)
        throw new InputError("limit_exceeded");
    }
  }
  #writable(state: SessionState): void {
    if (
      this.#owned.get(namespaceKey(state.session.namespace)) !==
      activeStage(state.session).binding.generation
    )
      throw new InputError("stale_binding");
  }
  #save(before: SessionState, after: SessionState): void {
    this.#checkCapacity(after);
    const params = nsValues(after.session.namespace);
    for (const g of after.generations)
      if (!before.generations.has(g))
        this.#db
          .prepare(`INSERT INTO generations (${scope},id) VALUES (?,?,?,?,?)`)
          .run(...params, g);
    const maps = [
      "commands",
      "interactions",
      "surfaces",
      "deliveries",
    ] as const;
    const save = (table: Table, id: string, value: WireRecord) =>
      this.#db
        .prepare(
          `INSERT INTO ${table} (${scope},id,json) VALUES (?,?,?,?,?,?) ON CONFLICT (${scope},id) DO UPDATE SET json=excluded.json`,
        )
        .run(...params, id, boundedJson(value, defaultLimits));
    // Commands precede referenced events/callbacks; deliveries follow their event.
    for (const [id, c] of after.commands)
      if (JSON.stringify(before.commands.get(id)) !== JSON.stringify(c))
        save("commands", id, c);
    for (const event of after.events.slice(before.events.length))
      this.#db
        .prepare(`INSERT INTO events (${scope},id,json) VALUES (?,?,?,?,?,?)`)
        .run(...params, event.eventId, boundedJson(event, defaultLimits));
    for (const table of maps.slice(1))
      for (const [id, row] of after[table])
        if (JSON.stringify(before[table].get(id)) !== JSON.stringify(row))
          save(table, id, row);
    // Deliberately last: a CAS/constraint failure rolls back every projection.
    const updated = this.#db
      .prepare(
        `UPDATE sessions SET json=? WHERE ${whereScope} AND revision=? AND generation IS ?`,
      )
      .run(
        boundedJson(after.session, defaultLimits),
        ...params,
        before.session.revision,
        before.session.currentStageId
          ? activeStage(before.session).binding.generation
          : null,
      );
    if (updated.changes !== 1) throw new InputError("stale_binding");
  }
  #rows(sql: string, params: SQLInputValue[]) {
    const size = this.#db
      .prepare(
        `SELECT coalesce(sum(length(CAST(json AS BLOB))),0) AS bytes FROM (${sql})`,
      )
      .get(...params)!;
    if (Number(size.bytes) > this.#bounds.maxQueryBytes)
      throw new InputError("limit_exceeded");
    return this.#db.prepare(sql).all(...params);
  }
  #bounded<T>(value: T): Result<T> {
    boundedJson(value, {
      maxBytes: this.#bounds.maxQueryBytes,
      maxTextBytes: this.#bounds.maxQueryBytes,
      maxDepth: 64,
      maxNodes: 1000000,
    });
    return ok(value);
  }
  #caller(caller: Caller): string[] {
    namespaceKey({ ...caller, sessionId: "caller-validation" });
    return [caller.tenantId, caller.principalId, caller.authorityId];
  }
  #connections(caller: Caller): Connection[] {
    return this.#rows(
      `SELECT json FROM connections c WHERE tenant_id=? AND principal_id=? AND authority_id=? AND revision=(SELECT max(revision) FROM connections r WHERE r.tenant_id=c.tenant_id AND r.principal_id=c.principal_id AND r.authority_id=c.authority_id AND r.id=c.id) ORDER BY id`,
      this.#caller(caller),
    ).map((row) => this.#decode<Connection>(row.json, "connection"));
  }
  #connection(
    caller: Caller,
    id: Id,
    revision?: number,
  ): Connection | undefined {
    if (!isId(id) || (revision !== undefined && !counter(revision)))
      throw new InputError("invalid_input");
    const row = this.#db
      .prepare(
        `SELECT json FROM connections WHERE tenant_id=? AND principal_id=? AND authority_id=? AND id=? ${revision === undefined ? "ORDER BY revision DESC LIMIT 1" : "AND revision=?"}`,
      )
      .get(
        ...this.#caller(caller),
        id,
        ...(revision === undefined ? [] : [revision]),
      );
    return row ? this.#decode<Connection>(row.json, "connection") : undefined;
  }
  async connections(caller: Caller): Promise<Result<readonly Connection[]>> {
    return this.#query(() =>
      this.#bounded(
        this.#connections(caller).filter((row) => row.status !== "deleted"),
      ),
    );
  }
  async connection(
    caller: Caller,
    id: Id,
    revision?: number,
  ): Promise<Result<Connection>> {
    return this.#query(() => {
      const row = this.#connection(caller, id, revision);
      return row ? ok(row) : fail("connection_required");
    });
  }
  #preferences(caller: Caller): UserPreferences {
    const row = this.#db
      .prepare(
        "SELECT json FROM preferences WHERE tenant_id=? AND principal_id=? AND authority_id=?",
      )
      .get(...this.#caller(caller));
    return row
      ? this.#decode<UserPreferences>(row.json, "userPreferences")
      : emptyPreferences();
  }
  #savePreferences(caller: Caller, prefs: UserPreferences): void {
    this.#db
      .prepare(
        "INSERT INTO preferences (tenant_id,principal_id,authority_id,json) VALUES (?,?,?,?) ON CONFLICT (tenant_id,principal_id,authority_id) DO UPDATE SET json=excluded.json",
      )
      .run(...this.#caller(caller), boundedJson(prefs, defaultLimits));
  }
  async preferences(caller: Caller): Promise<Result<UserPreferences>> {
    return this.#query(() => ok(this.#preferences(caller)));
  }
  async savePreferences(
    caller: Caller,
    patch: PreferencesPatch,
  ): Promise<Result<UserPreferences>> {
    return this.#transaction(() => {
      const merged = mergePreferences(this.#preferences(caller), patch);
      if (!merged.ok) return merged;
      const prefs = merged.value;
      if (
        prefs.defaultConnectionId &&
        this.#connection(caller, prefs.defaultConnectionId)?.status !== "ready"
      )
        return fail("connection_required");
      if (prefs.selectedSessionId)
        this.#session({ ...caller, sessionId: prefs.selectedSessionId });
      this.#savePreferences(caller, prefs);
      return ok(structuredClone(prefs));
    });
  }
  async saveConnection(
    caller: Caller,
    next: Connection,
    expected: number | null,
  ): Promise<Result<Connection>> {
    return this.#transaction(() => {
      const previous = this.#connection(caller, next.connectionId);
      const checked = connectionRevision(next, previous, expected);
      if (!checked.ok) return checked;
      const rows = this.#connections(caller);
      if (!previous && rows.length >= 128) return fail("limit_exceeded");
      const prefs = this.#preferences(caller);
      if (
        next.status === "ready" &&
        !prefs.defaultConnectionId &&
        !rows.some((row) => row.status === "ready")
      )
        prefs.defaultConnectionId = next.connectionId;
      if (
        next.status === "deleted" &&
        prefs.defaultConnectionId === next.connectionId
      )
        delete prefs.defaultConnectionId;
      this.#db
        .prepare(
          "INSERT INTO connections (tenant_id,principal_id,authority_id,id,revision,json) VALUES (?,?,?,?,?,?)",
        )
        .run(
          ...this.#caller(caller),
          next.connectionId,
          next.configRevision,
          boundedJson(next, defaultLimits),
        );
      this.#savePreferences(caller, prefs);
      return checked;
    });
  }
  async selectConnection(
    namespace: Namespace,
    connectionId: Id,
    revision: number,
    freshContext = false,
  ): Promise<Result<Session>> {
    return this.#transaction(() => {
      if (this.#connection(namespace, connectionId)?.status !== "ready")
        return fail("connection_required");
      const before = this.#state(namespace),
        result = selectConnection(before, connectionId, revision, freshContext);
      if (!result.ok) return result;
      if (result.value !== before) this.#save(before, result.value);
      return ok(result.value.session);
    });
  }
  async activateStage(input: StageActivation): Promise<Result<Session>> {
    const result = this.#transaction(() => {
      const before = this.#state(input.namespace),
        result = activateStage(before, input);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(result.value.session);
    });
    if (result.ok)
      this.#owned.set(
        namespaceKey(input.namespace),
        activeStage(result.value).binding.generation,
      );
    return result;
  }
  async create(session: Session): Promise<Result<void>> {
    const result = this.#transaction(() => {
      const state = createState(session);
      if (!state.ok) return state;
      this.#checkCapacity(state.value);
      const params = nsValues(session.namespace);
      if (
        this.#db
          .prepare(
            `SELECT 1 FROM sessions WHERE ${whereScope} UNION ALL SELECT 1 FROM tombstones WHERE ${whereScope}`,
          )
          .get(...params, ...params)
      )
        return fail("content_conflict");
      this.#db
        .prepare(`INSERT INTO sessions (${scope},json) VALUES (?,?,?,?,?)`)
        .run(...params, boundedJson(session, defaultLimits));
      if (session.currentStageId)
        this.#db
          .prepare(`INSERT INTO generations (${scope},id) VALUES (?,?,?,?,?)`)
          .run(...params, activeStage(session).binding.generation);
      return ok(undefined);
    });
    if (result.ok && session.currentStageId)
      this.#owned.set(
        namespaceKey(session.namespace),
        activeStage(session).binding.generation,
      );
    return result;
  }
  async session(n: Namespace): Promise<Result<Session>> {
    return this.#query(() => this.#bounded(this.#session(n)));
  }
  async command(n: Namespace, id: Id): Promise<Result<CommandRecord>> {
    return this.#query(() => {
      this.#session(n);
      if (!isId(id)) return fail("invalid_input");
      const row = this.#db
        .prepare(`SELECT json FROM commands WHERE ${whereScope} AND id=?`)
        .get(...nsValues(n), id);
      return row
        ? this.#bounded(this.#decode<CommandRecord>(row.json, "commandRecord"))
        : fail("unavailable");
    });
  }
  async delivery(
    n: Namespace,
    id: Id,
  ): Promise<Result<{ delivery: Delivery; event: Event } | null>> {
    return this.#query(() => {
      this.#session(n);
      if (!isId(id)) return fail("invalid_input");
      const row = this.#db
        .prepare(`SELECT json FROM deliveries WHERE ${whereScope} AND id=?`)
        .get(...nsValues(n), id);
      if (!row) return ok(null);
      const delivery = this.#decode<Delivery>(row.json, "delivery");
      const event = this.#db
        .prepare(`SELECT json FROM events WHERE ${whereScope} AND id=?`)
        .get(...nsValues(n), delivery.eventId);
      if (!event) return fail("invalid_input");
      return this.#bounded({
        delivery,
        event: this.#decode<Event>(event.json, "event"),
      });
    });
  }
  async surface(n: Namespace, id: Id): Promise<Result<SurfaceState>> {
    return this.#query(() => {
      if (this.#session(n).status !== "active") return fail("session_gone");
      if (!isId(id)) return fail("invalid_input");
      const row = this.#db
        .prepare(`SELECT json FROM surfaces WHERE ${whereScope} AND id=?`)
        .get(...nsValues(n), id);
      return row
        ? this.#bounded(this.#decode<SurfaceState>(row.json, "surface"))
        : fail("stale_binding");
    });
  }
  async accept(input: AcceptCommand): Promise<Result<Receipt>> {
    return this.#transaction(() => {
      const before = this.#state(input.namespace),
        result = acceptCommand(before, input);
      if (!result.ok) return result;
      if (result.value.state !== before) {
        this.#writable(before);
        this.#save(before, result.value.state);
      }
      return ok(result.value.receipt);
    });
  }
  async commit(batch: SessionCommit): Promise<Result<void>> {
    return this.#transaction(() => {
      if (
        [
          batch.commands,
          batch.events,
          batch.interactions,
          batch.surfaces,
          batch.deliveries,
          batch.providerFacts ?? [],
        ].some((rows) => !Array.isArray(rows)) ||
        batch.commands.length +
          batch.events.length +
          batch.interactions.length +
          batch.surfaces.length +
          batch.deliveries.length +
          (batch.providerFacts?.length ?? 0) >
          this.#bounds.maxBatchRecords
      )
        return fail("limit_exceeded");
      const before = this.#state(batch.namespace);
      if (!isDeliveryCommit(before, batch)) this.#writable(before);
      const result = commitSession(before, batch);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(undefined);
    });
  }
  async rebind(input: SessionRebind): Promise<Result<Session>> {
    const result = this.#transaction(() => {
      const before = this.#state(input.namespace),
        result = rebindSession(before, input);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(result.value.session);
    });
    if (result.ok)
      this.#owned.set(
        namespaceKey(input.namespace),
        activeStage(result.value).binding.generation,
      );
    return result;
  }
  async suspend(input: SessionSuspension): Promise<Result<Session>> {
    const result = this.#transaction(() => {
      const before = this.#state(input.namespace),
        result = suspendSession(before, input);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(result.value.session);
    });
    if (result.ok) this.#owned.delete(namespaceKey(input.namespace));
    return result;
  }
  async recoverUnavailable(
    input: RecoveryUnavailable,
  ): Promise<Result<Session>> {
    const result = this.#transaction(() => {
      const before = this.#state(input.namespace),
        result = recoverUnavailable(before, input);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(result.value.session);
    });
    if (result.ok) this.#owned.delete(namespaceKey(input.namespace));
    return result;
  }
  async reserveLaunch(launch: WorkerLaunch): Promise<Result<void>> {
    return this.#transaction(() => {
      validLaunch(launch);
      if (launch.phase !== "reserved") return fail("invalid_input");
      if (
        Number(
          this.#db.prepare("SELECT count(*) AS n FROM worker_launches").get()!
            .n,
        ) >= 128
      )
        return fail("limit_exceeded");
      if (
        this.#db
          .prepare(`SELECT 1 FROM worker_launches WHERE ${whereScope}`)
          .get(...nsValues(launch.namespace))
      )
        return fail("content_conflict");
      this.#db
        .prepare(
          `INSERT INTO worker_launches (${scope},launch_id,json) VALUES (?,?,?,?,?,?)`,
        )
        .run(
          ...nsValues(launch.namespace),
          launch.launchId,
          JSON.stringify(launch),
        );
      return ok(undefined);
    });
  }
  async registerLaunch(
    namespace: Namespace,
    launchId: Id,
    rootPid: number,
    pgid: number,
  ): Promise<Result<void>> {
    return this.#transaction(() => {
      const row = this.#db
        .prepare(
          `SELECT json FROM worker_launches WHERE ${whereScope} AND launch_id=?`,
        )
        .get(...nsValues(namespace), launchId);
      if (!row) return fail("stale_binding");
      const old = JSON.parse(String(row.json)) as WorkerLaunch;
      if (old.phase !== "reserved") return fail("content_conflict");
      const next: WorkerLaunch = { ...old, phase: "registered", rootPid, pgid };
      validLaunch(next);
      this.#db
        .prepare(
          `UPDATE worker_launches SET json=? WHERE ${whereScope} AND launch_id=?`,
        )
        .run(JSON.stringify(next), ...nsValues(namespace), launchId);
      return ok(undefined);
    });
  }
  async releaseLaunch(
    namespace: Namespace,
    launchId: Id,
  ): Promise<Result<void>> {
    return this.#transaction(() => {
      const result = this.#db
        .prepare(
          `DELETE FROM worker_launches WHERE ${whereScope} AND launch_id=?`,
        )
        .run(...nsValues(namespace), launchId);
      return Number(result.changes) === 1
        ? ok(undefined)
        : fail("stale_binding");
    });
  }
  async launches(): Promise<Result<readonly WorkerLaunch[]>> {
    return this.#query(() => {
      const rows = this.#db
        .prepare(
          "SELECT json FROM worker_launches ORDER BY launch_id LIMIT 129",
        )
        .all();
      if (rows.length > 128) return fail("limit_exceeded");
      return ok(
        rows.map((row) => {
          const launch = JSON.parse(String(row.json)) as WorkerLaunch;
          validLaunch(launch);
          return launch;
        }),
      );
    });
  }
  async snapshotPage(
    n: Namespace,
    query: PageQuery,
  ): Promise<Result<SnapshotPage>> {
    return this.#query(() => {
      const key = namespaceKey(n);
      this.#session(n);
      return readSnapshotPage(
        this.#views,
        `snapshot:${key}`,
        query,
        () => {
          const state = this.#state(n);
          return {
            session: state.session,
            records: [
              ...state.events,
              ...state.commands.values(),
              ...state.interactions.values(),
              ...state.surfaces.values(),
            ],
          };
        },
        {
          ...defaultLimits,
          maxBytes: Math.min(
            defaultLimits.maxBytes,
            this.#bounds.maxQueryBytes,
          ),
          maxTextBytes: Math.min(
            defaultLimits.maxTextBytes,
            this.#bounds.maxQueryBytes,
          ),
        },
      );
    });
  }
  async listSessions(
    caller: Caller,
    query: PageQuery,
  ): Promise<Result<SessionPage>> {
    return this.#query(() => {
      namespaceKey({ ...caller, sessionId: "scope-validation" });
      const params = [caller.tenantId, caller.principalId, caller.authorityId];
      return readSessionPage(
        this.#views,
        `list:${JSON.stringify(params)}`,
        query,
        () =>
          this.#rows(
            "SELECT json FROM sessions WHERE tenant_id=? AND principal_id=? AND authority_id=? AND status IN ('active','recovery_required') ORDER BY session_id",
            params,
          ).map((row) => {
            const session = this.#decode<Session>(row.json, "session");
            if (
              session.namespace.tenantId !== caller.tenantId ||
              session.namespace.principalId !== caller.principalId ||
              session.namespace.authorityId !== caller.authorityId
            )
              throw new SchemaError();
            return session;
          }),
        {
          ...defaultLimits,
          maxBytes: Math.min(
            defaultLimits.maxBytes,
            this.#bounds.maxQueryBytes,
          ),
          maxTextBytes: Math.min(
            defaultLimits.maxTextBytes,
            this.#bounds.maxQueryBytes,
          ),
        },
      );
    });
  }
  async events(
    n: Namespace,
    after: Counter,
    limit: number,
  ): Promise<Result<readonly Event[]>> {
    return this.#query(() => {
      const session = this.#session(n);
      if (!counter(after) || after > session.lastSequence)
        return fail("cursor_expired");
      if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1024)
        return fail("invalid_input");
      const rows = this.#rows(
        `SELECT json FROM events WHERE ${whereScope} AND sequence>? ORDER BY sequence LIMIT ?`,
        [...nsValues(n), after, limit],
      );
      return this.#bounded(
        rows.map((r) => this.#decode<Event>(r.json, "event")),
      );
    });
  }
  #page<T extends CommandRecord | Delivery>(
    table: "commands" | "deliveries",
    limit: number,
    after: string | undefined,
    due?: number,
  ): Result<Page<T>> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1024)
      return fail("invalid_input");
    let cursor: string[] | undefined;
    if (after !== undefined) {
      if (
        typeof after !== "string" ||
        after.length < 1 ||
        after.length > 2048 ||
        !/^[A-Za-z0-9_-]+$/.test(after)
      )
        return fail("invalid_input");
      try {
        cursor = JSON.parse(Buffer.from(after, "base64url").toString("utf8"));
      } catch {
        return fail("invalid_input");
      }
      if (!Array.isArray(cursor) || cursor.length !== 5 || !cursor.every(isId))
        return fail("invalid_input");
    }
    const filter =
      table === "commands"
        ? "state NOT IN ('terminal','invalidated','acknowledged','cancelled')"
        : "status!='delivered' AND due<=?";
    const params: SQLInputValue[] = table === "commands" ? [] : [due!];
    if (cursor) params.push(...cursor);
    const rows = this.#rows(
      `SELECT ${scope},id,json FROM ${table} WHERE ${filter}${cursor ? ` AND (${scope},id)>(?,?,?,?,?)` : ""} ORDER BY ${scope},id LIMIT ?`,
      [...params, limit + 1],
    );
    const items = rows
      .slice(0, limit)
      .map((r) =>
        this.#decode<T>(
          r.json,
          table === "commands" ? "commandRecord" : "delivery",
        ),
      );
    const last = rows[Math.min(rows.length, limit) - 1];
    return this.#bounded({
      items,
      ...(rows.length > limit
        ? {
            next: Buffer.from(
              JSON.stringify([
                last.tenant_id,
                last.principal_id,
                last.authority_id,
                last.session_id,
                last.id,
              ]),
            ).toString("base64url"),
          }
        : {}),
    });
  }
  async recovery(
    limit: number,
    after?: StoreCursor,
  ): Promise<Result<Page<CommandRecord>>> {
    return this.#query(() => this.#page("commands", limit, after));
  }
  async deliveries(
    limit: number,
    nowMs: Counter,
    after?: StoreCursor,
  ): Promise<Result<Page<Delivery>>> {
    return this.#query(() =>
      counter(nowMs)
        ? this.#page("deliveries", limit, after, nowMs)
        : fail("invalid_input"),
    );
  }
  async retire(
    n: Namespace,
    revision: Counter,
    generation: Id,
  ): Promise<Result<void>> {
    const retired = this.#transaction(() => {
      const before = this.#state(n);
      this.#writable(before);
      const result = retireSession(before, revision, generation);
      if (!result.ok) return result;
      this.#save(before, result.value);
      return ok(undefined);
    });
    if (retired.ok) this.#owned.delete(namespaceKey(n));
    return retired;
  }
  async pruneRetired(nowMs: Counter): Promise<Result<number>> {
    return this.#transaction(() => {
      if (!counter(nowMs)) return fail("invalid_input");
      const match = (alias: string) =>
        scope
          .split(", ")
          .map((k) => `${alias}.${k}=s.${k}`)
          .join(" AND ");
      const rows = this.#db
        .prepare(
          `SELECT ${scope} FROM sessions s WHERE status='retired'
        AND NOT EXISTS (SELECT 1 FROM commands c WHERE ${match("c")} AND receipt_until>=?)
        AND NOT EXISTS (SELECT 1 FROM deliveries d WHERE ${match("d")} AND status!='delivered') LIMIT ?`,
        )
        .all(nowMs, this.#bounds.maxBatchRecords);
      for (const row of rows) {
        const params = scope.split(", ").map((k) => row[k]);
        this.#db
          .prepare(`INSERT INTO tombstones (${scope}) VALUES (?,?,?,?)`)
          .run(...params);
        // FK references between child tables are immediate: delete dependants first.
        for (const table of [
          "deliveries",
          "surfaces",
          "interactions",
          "events",
          "commands",
          "generations",
          "sessions",
        ])
          this.#db
            .prepare(`DELETE FROM ${table} WHERE ${whereScope}`)
            .run(...params);
      }
      return ok(rows.length);
    });
  }
  async close(budget: Budget): Promise<Result<void>> {
    if (this.#closed) return ok(undefined);
    this.#closing = true;
    if (
      !Number.isSafeInteger(budget?.timeoutMs) ||
      budget.timeoutMs < 1 ||
      budget.timeoutMs > 2147483647 ||
      !(budget.signal instanceof AbortSignal)
    )
      return fail("invalid_input");
    if (budget.signal.aborted) return fail("unavailable", "same_command");
    try {
      // DatabaseSync cannot be preempted by an event-loop timer. Once entered,
      // native close runs to completion; only success means the lock was released.
      this.#db.close();
      this.#closed = true;
      this.#owned.clear();
      this.#views.clear();
      return ok(undefined);
    } catch (error) {
      return errorResult(error);
    }
  }
}
