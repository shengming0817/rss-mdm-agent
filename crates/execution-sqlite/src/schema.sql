CREATE TABLE metadata (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    authority BLOB NOT NULL,
    clock_watermark INTEGER NOT NULL CHECK(clock_watermark>=0)
);
CREATE TABLE executions (
    scope TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, plan_id TEXT NOT NULL UNIQUE,
    plan BLOB NOT NULL, digest TEXT NOT NULL, snapshot BLOB NOT NULL,
    revision INTEGER NOT NULL CHECK(revision>=0), reserve INTEGER NOT NULL CHECK(reserve BETWEEN 0 AND 15)
);
CREATE TABLE receipts (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE, event_id TEXT NOT NULL UNIQUE, scope TEXT NOT NULL, fingerprint TEXT NOT NULL,
    kind TEXT NOT NULL, body BLOB NOT NULL
);
CREATE INDEX receipts_scope_sequence ON receipts(scope,sequence);
CREATE TABLE audits (
    sequence INTEGER PRIMARY KEY REFERENCES receipts(sequence), body BLOB NOT NULL
);
CREATE TABLE event_keys (
    event_id TEXT PRIMARY KEY, scope TEXT NOT NULL,
    operation_id TEXT NOT NULL REFERENCES receipts(operation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE attempts (
    attempt_id TEXT PRIMARY KEY, scope TEXT NOT NULL REFERENCES executions(scope),
    operation_id TEXT NOT NULL REFERENCES receipts(operation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE interactions (
    id TEXT PRIMARY KEY, scope TEXT NOT NULL REFERENCES executions(scope),
    snapshot BLOB NOT NULL, revision INTEGER NOT NULL CHECK(revision BETWEEN 0 AND 1),
    reserve INTEGER NOT NULL CHECK(reserve BETWEEN 0 AND 1)
);
CREATE TABLE interaction_keys (
    command_id TEXT PRIMARY KEY, scope TEXT NOT NULL,
    operation_id TEXT NOT NULL REFERENCES receipts(operation_id) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE trust_heads (
    scope TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision>=1),
    authorization_revision BLOB NOT NULL, approval_revision BLOB NOT NULL,
    approval_digest TEXT NOT NULL, fresh_until INTEGER NOT NULL CHECK(fresh_until>=0)
);
CREATE TABLE trust_versions (
    scope TEXT NOT NULL, kind TEXT NOT NULL, version BLOB NOT NULL,
    PRIMARY KEY(scope,kind,version)
);
CREATE TABLE approvals (
    scope TEXT NOT NULL, record_id TEXT NOT NULL, version TEXT NOT NULL, definition BLOB NOT NULL,
    PRIMARY KEY(scope,record_id,version)
);
CREATE TABLE approval_heads (
    scope TEXT NOT NULL, record_id TEXT NOT NULL, version TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active','revoked','unknown')),
    PRIMARY KEY(scope,record_id),
    FOREIGN KEY(scope,record_id,version) REFERENCES approvals(scope,record_id,version)
);
CREATE TABLE approval_usage (
    scope TEXT NOT NULL, record_id TEXT NOT NULL, version TEXT NOT NULL,
    used INTEGER NOT NULL CHECK(used>=0), revision INTEGER NOT NULL CHECK(revision>=0),
    PRIMARY KEY(scope,record_id,version),
    FOREIGN KEY(scope,record_id,version) REFERENCES approvals(scope,record_id,version)
);
CREATE TABLE approval_consumptions (
    attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id), scope TEXT NOT NULL,
    record_id TEXT NOT NULL, version TEXT NOT NULL,
    PRIMARY KEY(attempt_id,scope,record_id,version),
    FOREIGN KEY(scope,record_id,version) REFERENCES approval_usage(scope,record_id,version)
);
CREATE TABLE confirmations (
    scope TEXT NOT NULL, consumer TEXT NOT NULL,
    sequence INTEGER NOT NULL REFERENCES receipts(sequence),
    PRIMARY KEY(scope,consumer,sequence)
);
