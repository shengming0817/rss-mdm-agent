import type { SelfServiceDirectoryItem, SelfServicePort } from "./types";

/** One current-subject directory; cursors are discovery positions, never permissions. */
export function createDirectory(
  port: Pick<SelfServicePort, "policy_directory" | "policy_resolve"> | null,
) {
  const state = {
    items: [] as SelfServiceDirectoryItem[],
    next: null as string | null,
    complete: false,
    windowed: false,
    loading: false,
    error: "",
    asOf: 0,
    readAt: 0,
  };
  const visited = new Set<string>();
  let active = true,
    epoch = 0;
  async function load(append = false) {
    if (!active || !port || state.loading || (append && !state.next)) return;
    const call = epoch,
      after = append ? state.next : null;
    state.loading = true;
    try {
      if (append && visited.size >= 2048) throw Error("capacity");
      const page = await port.policy_directory({ after });
      if (!active || call !== epoch) return;
      if (
        page.nextCursor &&
        (page.nextCursor === after || (append && visited.has(page.nextCursor)))
      )
        throw Error("cursor");
      if (!append) {
        visited.clear();
        state.windowed = false;
      }
      const merged = new Map(
        (append ? state.items : []).map((item) => [item.policyId, item]),
      );
      for (const item of page.items) {
        if (
          !item.capability ||
          !item.policyId ||
          !item.policyRevision ||
          !item.policyVersion ||
          !item.subject ||
          !item.selfService ||
          !item.task
        )
          throw Error("incomplete directory interface");
        merged.set(item.policyId, item as SelfServiceDirectoryItem);
      }
      if (merged.size > 1024) state.windowed = true;
      state.items = [...merged.values()].slice(-1024);
      if (after) visited.add(after);
      state.next = page.nextCursor;
      state.complete = !page.nextCursor && !state.windowed;
      state.asOf = page.asOf;
      state.readAt = Date.now();
      state.error = "";
    } catch {
      if (active && call === epoch)
        state.error = "目录读取未完成；已加载条目保留，可重试本页。";
    } finally {
      if (active && call === epoch) state.loading = false;
    }
  }
  function dispose() {
    active = false;
    epoch++;
    state.items = [];
    state.next = null;
    state.complete = false;
    state.loading = false;
    state.error = "";
    state.asOf = 0;
    visited.clear();
  }
  async function resolve(id: string) {
    if (!port || !active) return;
    return port.policy_resolve({ kind: "id", id });
  }
  return { port, state, load, resolve, dispose };
}
export type DirectoryController = ReturnType<typeof createDirectory>;

/** A Resource-owned display projection; parameters and native credentials never enter it. */
export function policyResource(
  item: SelfServiceDirectoryItem,
  catalog: DirectoryController,
): import("../workspace/page-navigation").PageResource {
  const text = (value: unknown) => (typeof value === "string" ? value : "");
  return {
    id: `policy:${item.policyId}`,
    title: text(item.selfService.presentation.displayName) || item.policyId,
    description: text(item.selfService.presentation.description),
    version: item.policyVersion,
    status: `${item.task.kind} · 风险 ${item.selfService.riskLevel} · 当前目录可见，执行仍需服务核验`,
    source: "授权 Policy 目录",
    revision: item.policyId,
    validUntil: catalog.state.readAt + 30_000,
    verified:
      !catalog.state.error &&
      catalog.state.items.some((current) => current.policyId === item.policyId),
  };
}
