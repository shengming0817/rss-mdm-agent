// ref: Vue core packages/reactivity/src/computed.ts@v3.5.38; all execution state remains service-owned.
import { computed, reactive, type InjectionKey } from "vue";
import type {
  DataAccessPort,
  DataCursor,
  InstalledSoftware,
  LocalSoftwareAction,
  LocalSoftwarePrepared,
  LocalSoftwareStatus,
  QueryDescriptor,
  QueryStatus,
  SoftwarePage,
  SoftwarePort,
} from "../self-service/types";
export const installationKey = (item: InstalledSoftware) =>
  item.reference.installation;
const referenceKey = (item: InstalledSoftware) =>
  JSON.stringify(item.reference);
export const ended = (status: LocalSoftwareStatus) =>
  [
    "cancelled",
    "admissionDenied",
    "failedBeforeDispatch",
    "executionEnded",
    "verified",
  ].includes(status.execution.phase);
type Source = {
  descriptor: QueryDescriptor;
  page?: SoftwarePage;
  items: InstalledSoftware[];
  after: DataCursor | null;
  busy: boolean;
  error: string;
  seen: Set<string>;
};
export function createSoftware(
  data: DataAccessPort | null,
  port: SoftwarePort | null,
) {
  const state = reactive({
    sources: [] as Source[],
    now: Date.now(),
    directoryNext: null as string | null,
    directoryComplete: false,
    loading: false,
    error: "",
    windowed: false,
    operations: new Map<string, LocalSoftwareStatus>(),
    prepared: new Map<string, LocalSoftwarePrepared>(),
    requests: new Set<string>(),
    busy: new Set<string>(),
    historyNext: null as string | null,
    historyComplete: false,
    historyBusy: false,
    historyError: "",
    queries: new Map<string, QueryStatus>(),
  });
  let active = true,
    epoch = 0;
  const timer = setInterval(() => {
    if (active) state.now = Date.now();
  }, 1000);
  const seenDirectory = new Set<string>(),
    seenHistory = new Set<string>();
  function accept(value: LocalSoftwareStatus) {
    if (
      !state.operations.has(value.prepared.request) &&
      state.operations.size >= 512
    ) {
      const old = [...state.operations.values()].find(ended);
      if (!old) throw Error("operation window full");
      state.operations.delete(old.prepared.request);
      state.prepared.delete(old.prepared.request);
      state.requests.delete(old.prepared.request);
      state.historyComplete = false;
    }
    state.operations.set(value.prepared.request, value);
    state.prepared.set(value.prepared.request, value.prepared);
    state.requests.add(value.prepared.request);
  }
  const installed = computed(() => {
    const rows = new Map<string, InstalledSoftware>();
    for (const source of [...state.sources].sort(
      (a, b) => (a.page?.collectedAt ?? 0) - (b.page?.collectedAt ?? 0),
    ))
      for (const item of source.items) rows.set(installationKey(item), item);
    return [...rows.values()].filter(
      (item) =>
        ![...state.operations.values()].some(
          (result) =>
            JSON.stringify(result.prepared.choice.software) ===
              referenceKey(item) &&
            ended(result) &&
            result.detected?.kind === "absent",
        ),
    );
  });
  const summary = computed(() => ({
    items: installed.value,
    installed: installed.value.length,
    updates: installed.value.filter((item) =>
      item.actions.some(
        (a) => a.action === "update" && a.available && a.targetVersion,
      ),
    ).length,
    complete:
      state.directoryComplete &&
      !state.windowed &&
      state.sources.length > 0 &&
      state.sources.every(
        (s) =>
          s.page &&
          !s.after &&
          !s.page.stale &&
          s.page.collectedAt != null &&
          s.page.collectedAt * 1000 <= state.now &&
          state.now - s.page.collectedAt * 1000 <= 300_000 &&
          s.page.quality === "complete" &&
          s.page.enumerationComplete &&
          !s.error,
      ),
    read:
      state.directoryComplete &&
      state.sources.length > 0 &&
      state.sources.every((s) => !!s.page),
    loading: state.loading || state.sources.some((s) => s.busy),
    error: state.error || state.sources.find((s) => s.error)?.error || "",
    collectedAt: state.sources.reduce<number | null>(
      (time, s) =>
        s.page?.collectedAt == null
          ? time
          : time == null
            ? s.page.collectedAt
            : Math.min(time, s.page.collectedAt),
      null,
    ),
    sources: state.sources.length,
  }));
  async function read(source: Source, append = false) {
    if (!active || !port || source.busy || (append && !source.after)) return;
    const call = epoch;
    source.busy = true;
    source.error = "";
    const after = append ? source.after : null;
    try {
      const page = await port.read({
        read: {
          selection: { kind: "query", query: source.descriptor.reference },
          after,
          limit: 128,
          maxAgeSeconds: 300,
        },
      });
      if (!active || call !== epoch) return;
      if (
        JSON.stringify(page.query) !==
        JSON.stringify(source.descriptor.reference)
      )
        throw Error("query changed");
      const key = JSON.stringify(page.next);
      if (
        page.next &&
        (key === JSON.stringify(after) ||
          (append && (source.seen.has(key) || source.seen.size >= 2048)))
      )
        throw Error("cursor");
      if (!append) source.seen.clear();
      if (after) source.seen.add(JSON.stringify(after));
      source.items = append ? [...source.items, ...page.items] : page.items;
      if (source.items.length > 1024) {
        source.items = source.items.slice(-1024);
        state.windowed = true;
      }
      source.page = page;
      source.after = page.next;
    } catch {
      if (active && call === epoch)
        source.error = "清单读取未完成；已加载记录保留，请重试本页。";
    } finally {
      if (active && call === epoch) source.busy = false;
    }
  }
  async function discover(append = false) {
    if (!active || !data || state.loading || (append && !state.directoryNext))
      return;
    state.loading = true;
    const call = epoch;
    const after = append ? state.directoryNext : null;
    try {
      const page = await data.query_directory({
        after,
        limit: 128,
        policyPage: null,
      });
      if (!active || call !== epoch) return;
      if (
        page.next &&
        (page.next === after ||
          (append &&
            (seenDirectory.has(page.next) || seenDirectory.size >= 2048)))
      )
        throw Error("cursor");
      if (!append) {
        state.sources = [];
        seenDirectory.clear();
        state.windowed = false;
      }
      if (after) seenDirectory.add(after);
      for (const descriptor of page.items.filter((d) =>
        d.domains.some((x) =>
          [
            "software.installations",
            "software.receipts",
            "software.packages",
          ].includes(x),
        ),
      )) {
        if (
          state.sources.some(
            (s) =>
              JSON.stringify(s.descriptor.reference) ===
              JSON.stringify(descriptor.reference),
          )
        )
          continue;
        if (state.sources.length >= 128) throw Error("source capacity");
        state.sources.push({
          descriptor,
          items: [],
          after: null,
          busy: false,
          error: "",
          seen: new Set(),
        });
      }
      state.directoryNext = page.next;
      state.directoryComplete = !page.next;
      state.error = "";
      await Promise.all(
        state.sources.filter((s) => !s.page).map((s) => read(s)),
      );
    } catch {
      if (active && call === epoch)
        state.error = "软件来源读取未完成；可重试，当前结果仍保留。";
    } finally {
      if (active && call === epoch) state.loading = false;
    }
  }
  async function history(append = false) {
    if (!active || !port || state.historyBusy || (append && !state.historyNext))
      return;
    const call = epoch;
    const after = append ? state.historyNext : null;
    state.historyBusy = true;
    try {
      const page = await port.history({ after, limit: 64 });
      if (!active || call !== epoch) return;
      if (
        page.next &&
        (page.next === after ||
          (append && (seenHistory.has(page.next) || seenHistory.size >= 2048)))
      )
        throw Error("cursor");
      if (!append) seenHistory.clear();
      if (after) seenHistory.add(after);
      for (const value of page.items) accept(value);
      state.historyNext = page.next;
      state.historyComplete = !page.next;
      state.historyError = "";
    } catch {
      if (active && call === epoch)
        state.historyError = "操作记录读取未完成，请查询原请求。";
    } finally {
      if (active && call === epoch) state.historyBusy = false;
    }
  }
  async function query(request: string) {
    if (!active || !port) return;
    const call = epoch;
    const value = await port.status({ request });
    if (active && call === epoch) accept(value);
    return value;
  }
  async function prepare(item: InstalledSoftware, action: LocalSoftwareAction) {
    if (
      !active ||
      !port ||
      !item.actions.some((a) => a.action === action && a.available)
    )
      return;
    const key = installationKey(item);
    if (state.busy.has(key)) return;
    const existing = [...state.operations.values()].find(
      (v) =>
        JSON.stringify(v.prepared.choice.software) === referenceKey(item) &&
        !ended(v),
    );
    if (existing) return existing.prepared;
    const call = epoch;
    state.busy.add(key);
    const request = crypto.randomUUID();
    state.requests.add(request);
    try {
      const value = await port.prepare({
        request,
        software: item.reference,
        action,
      });
      if (!active || call !== epoch) return;
      state.prepared.set(request, value);
      await query(request);
      return value;
    } catch {
      // Even prepare's lost acknowledgement is resolved by this UUID or service history.
      if (active && call === epoch)
        state.error = `准备回执未确认，请查询原请求：${request}`;
      await query(request).catch(() => {});
    } finally {
      if (active && call === epoch) state.busy.delete(key);
    }
  }
  async function submit(prepared: LocalSoftwarePrepared) {
    if (!active || !port || state.busy.has(prepared.request)) return;
    const call = epoch;
    state.busy.add(prepared.request);
    try {
      const value = await port.submit({
        request: prepared.request,
        digest: prepared.digest,
      });
      if (active && call === epoch) accept(value);
    } catch {
      if (active && call === epoch)
        state.error = "提交回执未确认；请查询原请求，避免重复操作。";
    } finally {
      if (active && call === epoch) state.busy.delete(prepared.request);
    }
  }
  async function cancel(request: string) {
    if (!port) return;
    const call = epoch;
    const value = await port.cancel({ request });
    if (active && call === epoch) accept(value);
  }
  async function refreshSource(source: Source) {
    if (!data || !source.descriptor.available || source.busy) return;
    const existing = [...state.queries.values()].find(
      (q) =>
        JSON.stringify(q.query) ===
          JSON.stringify(source.descriptor.reference) &&
        !["completed", "failed", "cancelled"].includes(q.phase),
    );
    if (existing) return;
    const request = crypto.randomUUID();
    const call = epoch;
    state.queries.set(request, {
      request,
      query: source.descriptor.reference,
      phase: "accepted",
      cancelRequested: false,
      preparation: null,
    });
    try {
      const value = await data.query_submit({
        request,
        query: source.descriptor.reference,
        parameters: {},
      });
      if (active && call === epoch) state.queries.set(request, value);
    } catch {
      if (active && call === epoch)
        state.error = `刷新回执未确认，请查询原刷新请求：${request}`;
    }
  }
  async function queryRefresh(request: string) {
    if (!data || !active) return;
    const call = epoch;
    try {
      const value = await data.query_status({ request });
      if (!active || call !== epoch) return;
      state.queries.set(request, value);
      if (value.phase === "completed") {
        const source = state.sources.find(
          (s) =>
            JSON.stringify(s.descriptor.reference) ===
            JSON.stringify(value.query),
        );
        if (source) await read(source);
      }
    } catch {
      if (active && call === epoch)
        state.error = "无法核实原刷新请求；请继续查询，不要重新采集。";
    }
  }
  async function cancelRefresh(request: string) {
    if (!data || !active) return;
    const call = epoch;
    try {
      const value = await data.query_cancel({ request });
      if (active && call === epoch) state.queries.set(request, value);
    } catch {
      if (active && call === epoch)
        state.error = "取消刷新未确认；请查询原刷新请求。";
    }
  }
  function dispose() {
    active = false;
    clearInterval(timer);
    ++epoch;
    state.sources = [];
    state.operations.clear();
    state.prepared.clear();
    state.requests.clear();
    state.queries.clear();
    state.busy.clear();
    state.error = "";
    state.historyError = "";
  }
  return {
    state,
    port,
    installed,
    summary,
    discover,
    read,
    history,
    prepare,
    submit,
    query,
    cancel,
    accept,
    refreshSource,
    queryRefresh,
    cancelRefresh,
    dispose,
  };
}
export type SoftwareController = ReturnType<typeof createSoftware>;
export const softwareSummaryKey: InjectionKey<SoftwareController["summary"]> =
  Symbol("authorized-software-summary");
