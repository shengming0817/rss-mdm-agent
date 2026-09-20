import type { Connection, UserPreferences } from "./wire.js";
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
    if (
      previous
        ? next.credentialRevision !==
          previous.credentialRevision +
            Number(next.credentialRef !== previous.credentialRef)
        : next.credentialRevision !== 1
    )
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
