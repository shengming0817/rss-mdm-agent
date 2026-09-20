import type { Connection, UserPreferences, PreferencesPatch } from "./wire.js";
import type { Result } from "./ports.js";
import { boundedJson, decode } from "./codec.js";
import { defaultLimits, fail, ok } from "./results.js";

export const emptyPreferences = (): UserPreferences => ({
  schemaVersion: 5,
  kind: "userPreferences",
});
/** Append-only revisions keep accepted phases reproducible without retaining secrets. */
export function connectionRevision(
  next: Connection,
  previous: Connection | undefined,
  expected: number | null,
): Result<Connection> {
  try {
    if (
      decode(boundedJson(next, defaultLimits), defaultLimits).kind !==
      "connection"
    )
      return fail("invalid_input");
    if ((previous?.configRevision ?? null) !== expected)
      return fail("revision_conflict");
    if (
      previous?.status === "deleted" ||
      next.configRevision !== (previous?.configRevision ?? 0) + 1
    )
      return fail("invalid_input");
    if (!next.name.trim() || /\p{Cc}/u.test(next.name))
      return fail("invalid_input");
    if (next.source.type === "custom_api") {
      const url = new URL(next.source.apiUrl);
      if (
        (url.protocol !== "https:" &&
          !(
            url.protocol === "http:" &&
            ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
          )) ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        !next.source.model.trim()
      )
        return fail("invalid_input");
    } else if (next.provider === "deepseek") return fail("invalid_input");
    return ok(structuredClone(next));
  } catch {
    return fail("invalid_input");
  }
}

/** Store-owned field updates avoid read-modify-write races between independent UI actions. */
export function mergePreferences(
  current: UserPreferences,
  patch: PreferencesPatch,
): Result<UserPreferences> {
  try {
    decode(
      boundedJson(
        { schemaVersion: 5, kind: "preferencesRequest", patch },
        defaultLimits,
      ),
      defaultLimits,
    );
    const next = { ...current };
    for (const key of ["defaultConnectionId", "selectedSessionId"] as const) {
      const change = patch[key];
      if (!change) continue;
      if ("set" in change) next[key] = change.set;
      else delete next[key];
    }
    return ok(next);
  } catch {
    return fail("invalid_input");
  }
}
