CREATE TABLE metadata (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    authority BLOB NOT NULL,
    clock_watermark INTEGER NOT NULL CHECK(clock_watermark>=0)
);
CREATE TABLE executions (
    scope TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,
    plan BLOB NOT NULL, digest TEXT NOT NULL, snapshot BLOB NOT NULL,
    revision INTEGER NOT NULL CHECK(revision>=0), reserve INTEGER NOT NULL CHECK(reserve BETWEEN 0 AND 31)
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
CREATE TABLE confirmations (
    scope TEXT NOT NULL, consumer TEXT NOT NULL,
    sequence INTEGER NOT NULL REFERENCES receipts(sequence),
    PRIMARY KEY(scope,consumer,sequence)
);

CREATE TABLE process_evidence (
    attempt_id TEXT PRIMARY KEY REFERENCES attempts(attempt_id), body BLOB NOT NULL,
    stdout BLOB NOT NULL, stderr BLOB NOT NULL
);

CREATE TABLE resource_claims (
    resource TEXT PRIMARY KEY, attempt_id TEXT NOT NULL REFERENCES attempts(attempt_id)
);
CREATE TABLE software_progress (
    attempt_id TEXT PRIMARY KEY REFERENCES attempts(attempt_id), body BLOB NOT NULL
);

CREATE TABLE backend_requests (scope TEXT PRIMARY KEY, actor TEXT NOT NULL, request_id TEXT NOT NULL UNIQUE, body BLOB NOT NULL);

-- Derived lookup over the sole immutable plan; no second task identity authority.
CREATE INDEX executions_remote_identity ON executions (
    json_extract(CAST(plan AS TEXT),'$.request.initiator.task'),
    json_extract(CAST(plan AS TEXT),'$.request.initiator.attempt')
);
