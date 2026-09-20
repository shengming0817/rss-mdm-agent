/** Expected support is separate from the availability of a local CLI configuration. */
export function classifySource(row) {
  const unsupported =
    row.provider === "claude" && row.source === "existing_login";
  const expected = unsupported
    ? "unsupported_capability"
    : "model_probe_completed";
  return {
    ...row,
    expected,
    applicability: row.result === "source_absent" ? "source_absent" : "present",
    status:
      row.result === expected
        ? "passed"
        : !unsupported && row.result === "source_absent"
          ? "not_applicable"
          : "failed",
  };
}
export function sourceSummary(rows) {
  const classified = rows.map(classifySource);
  return {
    status: classified.some((row) => row.status === "failed")
      ? "failed"
      : classified.some((row) => row.status === "not_applicable")
        ? "partial"
        : "passed",
    results: classified,
  };
}
