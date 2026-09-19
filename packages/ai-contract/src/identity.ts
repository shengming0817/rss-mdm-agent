// @generated from schema/runtime.schema.json. Do not edit.
export const interactionCatalog = Object.freeze({
  version: "v0.9.1",
  catalogId: "urn:rss-mdm-agent:a2ui:interaction",
  catalogVersion: "1",
} as const);
export const errorCodes = Object.freeze([
  "invalid_input",
  "unsupported_version",
  "unsupported_capability",
  "permission_denied",
  "content_conflict",
  "revision_conflict",
  "stale_binding",
  "expired",
  "unavailable",
  "reconciliation_required",
  "limit_exceeded",
  "cursor_expired",
  "session_gone",
  "already_answered",
  "storage_corrupt",
] as const);
