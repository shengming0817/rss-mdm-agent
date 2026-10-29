import { deepClone, proxyMap, proxySet } from "valtio/vanilla/utils";
import { proxy, type Snapshot } from "valtio/vanilla";
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
export const installationKey = (item: Snapshot<InstalledSoftware>) =>
  item.reference.installation;
const referenceKey = (item: Snapshot<InstalledSoftware>) =>
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
export const canSubmitSoftware = (
  status: Snapshot<LocalSoftwareStatus> | undefined,
  now: number,
) =>
  !!status &&
  ["waiting", "confirmationRequired"].includes(status.execution.phase) &&
  status.execution.attempts === 0 &&
  !status.execution.cancelRequested &&
  status.execution.admission == null &&
  status.prepared.startBeforeUnixMs > now;
export function createSoftware(
  data: DataAccessPort | null,
  port: SoftwarePort | null,
  tasks?: import("../task-center/controller").Controller,
) {
  const state = proxy({
    sources: [] as Source[],
    now: Date.now(),
    directoryNext: null as string | null,
    directoryComplete: false,
    loading: false,
    error: "",
    windowed: false,
    operations: proxyMap<string, LocalSoftwareStatus>(),
    prepared: proxyMap<string, LocalSoftwarePrepared>(),
    requests: proxySet<string>(),
    busy: proxySet<string>(),
    uncertain: proxySet<string>(),
    historyNext: null as string | null,
    historyComplete: false,
    historyBusy: false,
    historyError: "",
    queries: proxyMap<string, QueryStatus>(),
  });
  type ReadState = Snapshot<typeof state>;
  let active = true,
    epoch = 0;
  const timer = setInterval(() => {
    if (active) state.now = Date.now();
  }, 1000);
  // Every asynchronous status/history read is fenced against a later write for its original request.
  let serial = 0;
  const latest = new Map<string, number>(),
    writes = new Set<string>();
  const begin = (request: string) => {
    const ticket = ++serial;
    latest.set(request, ticket);
    return ticket;
  };
  const seenDirectory = new Set<string>(),
    seenHistory = new Set<string>();
  function accept(value: LocalSoftwareStatus) {
    value = deepClone(value);
    if (
      !state.operations.has(value.prepared.request) &&
      state.operations.size >= 512
    ) {
      const old = [...state.operations.values()].find(ended);
      if (!old) throw Error("operation window full");
      state.operations.delete(old.prepared.request);
      state.prepared.delete(old.prepared.request);
      state.requests.delete(old.prepared.request);
      state.uncertain.delete(old.prepared.request);
      latest.delete(old.prepared.request);
      state.historyComplete = false;
    }
    state.operations.set(value.prepared.request, value);
    state.prepared.set(value.prepared.request, value.prepared);
    state.requests.add(value.prepared.request);
  }
  // Each physical inventory needs one complete observation; delivery identities
  // (local recipe, enterprise task or periodic policy) may cover the same inventory.
  const complete = (s: Snapshot<Source>) =>
    !!s.page &&
    !s.after &&
    !s.page.stale &&
    s.page.collectedAt != null &&
    s.page.quality === "complete" &&
    s.page.enumerationComplete &&
    !s.error;
  const observations = (current: ReadState = state) => {
    const selected = new Map<string, Snapshot<Source>>();
    for (const source of current.sources) {
      for (const coverage of source.page?.coverage ?? []) {
        const previous = selected.get(coverage);
        if (
          !previous ||
          (complete(source) && !complete(previous)) ||
          (complete(source) === complete(previous) &&
            (source.page?.collectedAt ?? 0) >=
              (previous.page?.collectedAt ?? 0))
        )
          selected.set(coverage, source);
      }
    }
    return selected;
  };
  const coverageComplete = (current: ReadState = state) =>
    current.sources.every(
      (s) =>
        s.page &&
        s.page.coverage.length > 0 &&
        s.page.coverage.every((key) => {
          const selected = observations(current).get(key);
          return selected && complete(selected);
        }),
    );
  const installed = (current: ReadState = state) => {
    const rows = new Map<string, Snapshot<InstalledSoftware>>();
    for (const source of [...current.sources].sort(
      (a, b) => (a.page?.collectedAt ?? 0) - (b.page?.collectedAt ?? 0),
    ))
      for (const item of source.items)
        if (observations(current).get(item.coverage) === source)
          rows.set(installationKey(item), item);
    // The inventory projection is replaced by its collection owner. Historical task
    // detections describe that task; their stable object references are not observation versions.
    return [...rows.values()];
  };
  const summary = (current: ReadState = state) => ({
    items: installed(current),
    installed: installed(current).length,
    updates: installed(current).filter((item) =>
      item.actions.some(
        (a) => a.action === "update" && a.available && a.targetVersion,
      ),
    ).length,
    complete:
      current.directoryComplete &&
      !current.windowed &&
      current.sources.length > 0 &&
      coverageComplete(current),
    read: current.sources.some(
      (s) =>
        s.page && ["complete", "partial", "truncated"].includes(s.page.quality),
    ),
    loading: current.loading || current.sources.some((s) => s.busy),
    error: current.error || current.sources.find((s) => s.error)?.error || "",
    collectedAt: [...new Set(observations(current).values())].reduce<
      number | null
    >(
      (time, s) =>
        s.page?.collectedAt == null
          ? time
          : time == null
            ? s.page.collectedAt
            : Math.min(time, s.page.collectedAt),
      null,
    ),
    sources: current.sources.length,
  });
  async function read(source: Source, append = false) {
    if (!active || !port || source.busy || (append && !source.after)) return;
    const call = epoch;
    source.busy = true;
    source.error = "";
    const after = append ? source.after : null;
    try {
      const page = deepClone(
        await port.read({
          read: {
            selection: { kind: "query", query: source.descriptor.reference },
            after,
            limit: 128,
            maxAgeSeconds: 300,
          },
        }),
      );
      if (!active || call !== epoch) return;
      if (
        JSON.stringify(page.query) !==
        JSON.stringify(source.descriptor.reference)
      )
        throw Error("query changed");
      if (page.items.some((item) => !page.coverage.includes(item.coverage)))
        throw Error("inventory coverage changed");
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
  let discovery: Promise<void> | undefined;
  function discover(
    append = false,
    visible: () => boolean = () => true,
  ): Promise<void> {
    if (discovery) return discovery;
    discovery = discoverPage(append, visible).finally(() => {
      discovery = undefined;
    });
    return discovery;
  }
  async function discoverPage(append: boolean, visible: () => boolean) {
    if (!active || !data || state.loading || (append && !state.directoryNext))
      return;
    state.loading = true;
    const call = epoch;
    const after = append ? state.directoryNext : null;
    try {
      const page = await data.query_directory({
        after,
        limit: 128,
        refreshPolicies: false,
        policyCursor: null,
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
          descriptor: deepClone(descriptor),
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
      for (const source of state.sources.filter((s) => !s.page)) {
        if (!visible()) break;
        await read(source);
      }
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
    const call = epoch,
      ticket = ++serial;
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
      for (const value of page.items) {
        const request = value.prepared.request;
        if (writes.has(request) || (latest.get(request) ?? 0) > ticket)
          continue;
        latest.set(request, ticket);
        accept(value);
        state.uncertain.delete(request);
      }
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
    if (!active || !port || writes.has(request)) return;
    const call = epoch,
      ticket = begin(request);
    const value = await port.status({ request });
    if (
      active &&
      call === epoch &&
      latest.get(request) === ticket &&
      !writes.has(request)
    ) {
      if (value.prepared.request !== request)
        throw Error("software request mismatch");
      accept(value);
      state.uncertain.delete(request);
      state.error = "";
      return value;
    }
  }
  async function prepare(
    item: Snapshot<InstalledSoftware>,
    action: LocalSoftwareAction,
  ) {
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
    begin(request);
    writes.add(request);
    try {
      const value = await port.prepare({
        request,
        software: item.reference,
        action,
      });
      if (!active || call !== epoch) return;
      state.prepared.set(request, value);
      writes.delete(request);
      await query(request);
      return value;
    } catch {
      // Even prepare's lost acknowledgement is resolved by this UUID or service history.
      if (active && call === epoch)
        state.error = `准备回执未确认，请查询原请求：${request}`;
      writes.delete(request);
      await query(request).catch(() => {});
    } finally {
      writes.delete(request);
      if (active && call === epoch) state.busy.delete(key);
    }
  }
  async function submit(prepared: LocalSoftwarePrepared) {
    if (
      !active ||
      !port ||
      state.busy.has(prepared.request) ||
      state.uncertain.has(prepared.request) ||
      !canSubmitSoftware(state.operations.get(prepared.request), Date.now())
    )
      return;
    const call = epoch,
      ticket = begin(prepared.request);
    writes.add(prepared.request);
    state.busy.add(prepared.request);
    try {
      const value = await port.submit({
        request: prepared.request,
        digest: prepared.digest,
      });
      if (active && call === epoch && latest.get(prepared.request) === ticket)
        accept(value);
    } catch {
      if (active && call === epoch) {
        state.uncertain.add(prepared.request);
        state.error = "提交回执未确认；请查询原请求，避免重复操作。";
      }
    } finally {
      writes.delete(prepared.request);
      if (active && call === epoch) state.busy.delete(prepared.request);
    }
  }
  async function cancel(request: string) {
    if (!active || !port || state.busy.has(request)) return;
    const call = epoch,
      ticket = begin(request);
    if (!tasks) throw Error("task port unavailable");
    writes.add(request);
    state.busy.add(request);
    try {
      await tasks.cancel(request);
      const value = await port.status({ request });
      if (active && call === epoch && latest.get(request) === ticket)
        accept(value);
    } finally {
      writes.delete(request);
      if (active && call === epoch) state.busy.delete(request);
    }
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
      if (!tasks) throw Error("task port unavailable");
      await tasks.cancel(request);
      const value = await data.query_status({ request });
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
    state.uncertain.clear();
    latest.clear();
    writes.clear();
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
