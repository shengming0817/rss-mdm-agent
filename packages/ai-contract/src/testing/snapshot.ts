import type { Result, SessionStore } from "../ports.js";
import type { Namespace, SnapshotPage } from "../wire.js";
/** Aggregate real pages for conformance assertions, checking one immutable view. */
export async function readSnapshot(
  store: SessionStore,
  namespace: Namespace,
): Promise<
  Result<
    Pick<
      SnapshotPage,
      "session" | "cursor" | "events" | "commands" | "interactions" | "surfaces"
    >
  >
> {
  const first = await store.snapshotPage(namespace, { limit: 256 });
  if (!first.ok) return first;
  const {
    snapshotId,
    pageIndex: _index,
    schemaVersion: _version,
    kind: _kind,
    next: _next,
    ...snapshot
  } = first.value;
  let page = first.value;
  const seen = new Set<string>();
  while (page.next) {
    if (seen.has(page.next)) throw new Error("Repeated snapshot continuation");
    seen.add(page.next);
    const result = await store.snapshotPage(namespace, {
      limit: 256,
      continuation: page.next,
    });
    if (!result.ok) return result;
    const next = result.value;
    if (
      next.snapshotId !== snapshotId ||
      next.cursor !== snapshot.cursor ||
      next.pageIndex !== page.pageIndex + 1
    )
      throw new Error("Snapshot view changed");
    for (const field of [
      "events",
      "commands",
      "interactions",
      "surfaces",
    ] as const)
      snapshot[field] = [...snapshot[field], ...next[field]] as never;
    page = next;
  }
  return { ok: true, value: snapshot };
}
