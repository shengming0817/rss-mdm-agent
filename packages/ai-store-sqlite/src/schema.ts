import { createHash } from "node:crypto";
import type { DatabaseSync } from "node:sqlite";

export const version = 2;
export const applicationId = 0x52534149;
export const scope = "tenant_id, principal_id, authority_id, session_id";
export const whereScope =
  "tenant_id=? AND principal_id=? AND authority_id=? AND session_id=?";
const columns =
  "tenant_id TEXT NOT NULL, principal_id TEXT NOT NULL, authority_id TEXT NOT NULL, session_id TEXT NOT NULL";
const sessionFk = `FOREIGN KEY (${scope}) REFERENCES sessions (${scope}) ON DELETE CASCADE`;
const row = "json TEXT NOT NULL CHECK(json_valid(json))";
// Independent product schema. Every relational key and reference carries all four
// authority dimensions. Native transcripts and Rust execution authority stay outside.
const statements = [
  "CREATE TABLE schema_meta (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL) STRICT",
  `CREATE TABLE sessions (${columns}, ${row},
    revision INTEGER GENERATED ALWAYS AS (json_extract(json,'$.revision')) STORED NOT NULL,
    generation TEXT GENERATED ALWAYS AS (json_extract(json,'$.binding.generation')) STORED NOT NULL,
    status TEXT GENERATED ALWAYS AS (json_extract(json,'$.status')) STORED NOT NULL,
    PRIMARY KEY (${scope})) STRICT, WITHOUT ROWID`,
  `CREATE TABLE generations (${columns}, id TEXT NOT NULL, PRIMARY KEY (${scope},id), ${sessionFk}) STRICT, WITHOUT ROWID`,
  `CREATE TABLE commands (${columns}, id TEXT NOT NULL, ${row},
    state TEXT GENERATED ALWAYS AS (json_extract(json,'$.state')) STORED NOT NULL,
    receipt_until INTEGER GENERATED ALWAYS AS (json_extract(json,'$.receipt.receiptUntilMs')) STORED NOT NULL,
    observer TEXT GENERATED ALWAYS AS (json_extract(json,'$.dispatch.observerGeneration')) STORED,
    PRIMARY KEY (${scope},id), ${sessionFk},
    FOREIGN KEY (${scope},observer) REFERENCES generations (${scope},id)) STRICT, WITHOUT ROWID`,
  `CREATE TABLE events (${columns}, id TEXT NOT NULL, ${row},
    sequence INTEGER GENERATED ALWAYS AS (json_extract(json,'$.sequence')) STORED NOT NULL,
    command_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.commandId')) STORED,
    generation TEXT GENERATED ALWAYS AS (json_extract(json,'$.generation')) STORED NOT NULL,
    PRIMARY KEY (${scope},id), UNIQUE (${scope},sequence), ${sessionFk},
    FOREIGN KEY (${scope},command_id) REFERENCES commands (${scope},id),
    FOREIGN KEY (${scope},generation) REFERENCES generations (${scope},id)) STRICT, WITHOUT ROWID`,
  `CREATE TABLE interactions (${columns}, id TEXT NOT NULL, ${row},
    command_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.commandId')) STORED NOT NULL,
    generation TEXT GENERATED ALWAYS AS (json_extract(json,'$.generation')) STORED NOT NULL,
    callback_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.nativeCallbackId')) STORED NOT NULL,
    response_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.responseCommandId')) STORED,
    PRIMARY KEY (${scope},id), UNIQUE (${scope},generation,callback_id), ${sessionFk},
    FOREIGN KEY (${scope},command_id) REFERENCES commands (${scope},id),
    FOREIGN KEY (${scope},response_id) REFERENCES commands (${scope},id),
    FOREIGN KEY (${scope},generation) REFERENCES generations (${scope},id)) STRICT, WITHOUT ROWID`,
  `CREATE TABLE surfaces (${columns}, id TEXT NOT NULL, ${row},
    interaction_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.interactionId')) STORED NOT NULL,
    generation TEXT GENERATED ALWAYS AS (json_extract(json,'$.generation')) STORED NOT NULL,
    PRIMARY KEY (${scope},id), ${sessionFk},
    FOREIGN KEY (${scope},interaction_id) REFERENCES interactions (${scope},id),
    FOREIGN KEY (${scope},generation) REFERENCES generations (${scope},id)) STRICT, WITHOUT ROWID`,
  `CREATE TABLE deliveries (${columns}, id TEXT NOT NULL, ${row},
    event_id TEXT GENERATED ALWAYS AS (json_extract(json,'$.eventId')) STORED NOT NULL,
    status TEXT GENERATED ALWAYS AS (json_extract(json,'$.status')) STORED NOT NULL,
    due INTEGER GENERATED ALWAYS AS (json_extract(json,'$.nextAttemptAtMs')) STORED NOT NULL,
    PRIMARY KEY (${scope},id), ${sessionFk},
    FOREIGN KEY (${scope},event_id) REFERENCES events (${scope},id)) STRICT, WITHOUT ROWID`,
  `CREATE TABLE tombstones (${columns}, PRIMARY KEY (${scope})) STRICT, WITHOUT ROWID`,
  `CREATE INDEX recovery_commands ON commands (state,${scope},id)`,
  `CREATE INDEX due_deliveries ON deliveries (status,due,${scope},id)`,
];
const normalize = (sql: string) => sql.replace(/\s+/g, " ").trim();
const normalizedStatements = statements.map(normalize).sort();
const checksum = createHash("sha256")
  .update(normalizedStatements.join(";\n"))
  .digest("hex");

/** Read-only identity and shape check, also rerun under the writer lock. */
export function validateExisting(db: DatabaseSync): void {
  if (
    Number(db.prepare("PRAGMA user_version").get()!.user_version) !== version ||
    Number(db.prepare("PRAGMA application_id").get()!.application_id) !==
      applicationId
  )
    throw new SchemaError("unsupported_version");
  const actual = db
    .prepare(
      "SELECT sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .all()
    .map((r) => normalize(String(r.sql)))
    .sort();
  if (JSON.stringify(actual) !== JSON.stringify(normalizedStatements))
    throw new SchemaError();
  const metadata = db.prepare("SELECT version,checksum FROM schema_meta").all();
  if (
    metadata.length !== 1 ||
    metadata[0].version !== version ||
    metadata[0].checksum !== checksum
  )
    throw new SchemaError();
  if (
    db.prepare("PRAGMA quick_check(1)").get()!.quick_check !== "ok" ||
    db.prepare("PRAGMA foreign_key_check").get()
  )
    throw new SchemaError();
}

/** Startup owns a real write transaction before advertising the store. Version 0
 * is accepted only by explicit create, never as recovery of a partial/foreign DB. */
export function initialize(db: DatabaseSync, create: boolean): void {
  const current = Number(db.prepare("PRAGMA user_version").get()!.user_version);
  const app = Number(db.prepare("PRAGMA application_id").get()!.application_id);
  if (
    create
      ? current !== 0 || app !== 0
      : current !== version || app !== applicationId
  )
    throw new SchemaError("unsupported_version");
  db.exec("BEGIN IMMEDIATE");
  try {
    if (create) {
      if (db.prepare("SELECT name FROM sqlite_schema LIMIT 1").get())
        throw new SchemaError();
      for (const sql of statements) db.exec(sql);
      db.prepare("INSERT INTO schema_meta VALUES (?,?)").run(version, checksum);
      db.exec(
        `PRAGMA application_id=${applicationId}; PRAGMA user_version=${version}`,
      );
    }
    validateExisting(db);
    // Exercise the writer lock even when no migration is needed. EXCLUSIVE/WAL
    // retains it between transactions until this connection closes or dies.
    db.prepare("UPDATE schema_meta SET checksum=checksum WHERE version=?").run(
      version,
    );
    db.exec("COMMIT");
  } catch (error) {
    if (db.isTransaction) db.exec("ROLLBACK");
    throw error;
  }
}
export class SchemaError extends Error {
  constructor(
    readonly code:
      | "unsupported_version"
      | "storage_corrupt" = "storage_corrupt",
  ) {
    super(`AI SQLite: ${code}`);
  }
}
