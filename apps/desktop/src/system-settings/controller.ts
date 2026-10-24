// ref: Vue core packages/reactivity/src/computed.ts@v3.5.38.
import { computed, reactive, type InjectionKey } from "vue";
import type {
  CollectedData,
  DataAccessPort,
  QueryDescriptor,
  SettingsOperation,
  SettingsPatch,
  SettingsResponse,
  SettingsSection,
} from "../self-service/types";
export type SettingsPort = (
  input: SettingsOperation,
) => Promise<SettingsResponse>;
const sections: SettingsSection[] = [
  "network",
  "display",
  "sound",
  "power",
  "lock",
  "storage",
  "security",
  "about",
];
const families: Record<SettingsSection, string[]> = {
  network: ["hardware.network"],
  display: [],
  sound: [],
  power: ["hardware.battery"],
  lock: ["hardware.battery"],
  storage: [
    "hardware.volumes",
    "hardware.containers",
    "hardware.disks",
    "hardware.os",
  ],
  security: ["security."],
  about: [
    "hardware.os",
    "hardware.cpu",
    "hardware.memory",
    "hardware.system",
    "hardware.identity",
  ],
};
export function createSystemSettings(
  port: SettingsPort | null,
  data: DataAccessPort | null,
  identity: () => string = () => crypto.randomUUID(),
) {
  const state = reactive({
    observations: new Map<SettingsSection, SettingsResponse>(),
    facts: new Map<string, { source: QueryDescriptor; value: CollectedData }>(),
    sources: [] as QueryDescriptor[],
    busy: new Set<SettingsSection>(),
    errors: new Map<SettingsSection, string>(),
    pending: new Map<
      SettingsSection,
      { request: string; operation: SettingsOperation }
    >(),
    now: Date.now(),
  });
  let active = true,
    epoch = 0;
  const relevant = (section: SettingsSection) =>
    state.sources.filter((s) =>
      s.domains.some((d) => families[section].some((p) => d.startsWith(p))),
    );
  const timer = setInterval(() => {
    if (active) state.now = Date.now();
  }, 250);
  const message = (error: unknown) => {
    const code =
      error && typeof error === "object" && "code" in error
        ? String(error.code)
        : "";
    return code.includes("conflict")
      ? "系统状态已变化，请刷新后重新选择。"
      : code.includes("denied")
        ? "当前用户或管理员权限不允许此操作。"
        : "未取得完整结果。请查询原任务，避免重复修改。";
  };
  function accept(section: SettingsSection, response: SettingsResponse) {
    if (!active) return;
    const pending = state.pending.get(section);
    if (!response.request && pending) return;
    state.observations.set(section, response);
    if (response.finished && response.request === pending?.request)
      state.pending.delete(section);
  }
  async function directory() {
    if (!data || !active || state.sources.length) return;
    let after: string | null = null;
    const sources: QueryDescriptor[] = [],
      seen = new Set<string>();
    for (let pages = 0; pages < 16; pages++) {
      const page = await data.query_directory({
        after,
        limit: 128,
        policyPage: null,
      });
      for (const source of page.items)
        if (source.kind === "native" && !seen.has(source.reference.id)) {
          seen.add(source.reference.id);
          sources.push(source);
        }
      if (!page.next) {
        if (active) state.sources = sources;
        return;
      }
      if (page.next === after) throw Error("directory continuation");
      after = page.next;
    }
    throw Error("directory bound");
  }
  async function readFacts(
    section: SettingsSection,
    refresh: boolean,
    call: number,
  ) {
    if (!data) return;
    await directory();
    await Promise.all(
      relevant(section).map(async (source) => {
        if (refresh && source.available) {
          const request = identity();
          let status = await data.query_submit({
            request,
            query: source.reference,
            parameters: {},
          });
          for (
            let n = 0;
            active &&
            call === epoch &&
            !["completed", "failed", "cancelled", "unknown"].includes(
              status.phase,
            ) &&
            n < 240;
            n++
          ) {
            await new Promise((resolve) => setTimeout(resolve, 250));
            if (!active || call !== epoch) return;
            status = await data.query_status({ request });
          }
        }
        if (!active || call !== epoch) return;
        const value = await data.data_read({
          selection: { kind: "query", query: source.reference },
          after: null,
          limit: 128,
          maxAgeSeconds: 300,
        });
        if (active && call === epoch)
          state.facts.set(source.reference.id, { source, value });
      }),
    );
  }
  async function poll(
    section: SettingsSection,
    request: string,
    call: number,
    publish = true,
  ) {
    if (!port) return;
    for (let n = 0; active && call === epoch && n < 300; n++) {
      const response = await port({ kind: "status", request });
      if (!active || call !== epoch) return;
      if (publish) accept(section, response);
      else if (response.finished && response.effect !== "openRequested")
        state.errors.set(
          section,
          response.reason ?? "系统未确认打开设置，请重试。",
        );
      if (response.finished) return;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  async function run(section: SettingsSection, operation: SettingsOperation) {
    if (!port || !active || state.busy.has(section)) return;
    const request = "request" in operation ? operation.request : undefined,
      call = epoch,
      navigation = operation.kind === "openNative";
    state.busy.add(section);
    if (!navigation) state.errors.delete(section);
    if (
      request &&
      !navigation &&
      !["status", "cancel", "keepDisplay"].includes(operation.kind)
    ) {
      const pending = state.pending.get(section);
      if (pending && pending.request !== request) {
        state.busy.delete(section);
        state.errors.set(section, "仍有原任务结果未确认，请先查询原任务。");
        return;
      }
      state.pending.set(section, {
        request,
        operation: JSON.parse(JSON.stringify(operation)) as SettingsOperation,
      });
    }
    try {
      const response = await port(operation);
      if (!active || call !== epoch) return;
      if (!navigation) accept(section, response);
      else if (response.finished && response.effect !== "openRequested")
        state.errors.set(
          section,
          response.reason ?? "系统未确认打开设置，请重试。",
        );
      if (request && !response.finished)
        await poll(section, request, call, !navigation);
    } catch (error) {
      if (active && call === epoch) state.errors.set(section, message(error));
    } finally {
      if (active && call === epoch) state.busy.delete(section);
    }
  }
  async function load(section: SettingsSection, refresh = false) {
    if (!active || state.busy.has(section)) return;
    const call = epoch;
    const local = run(
      section,
      refresh &&
        ["network", "display", "sound", "power", "lock"].includes(section)
        ? { kind: "refresh", section, request: identity() }
        : { kind: "snapshot", section },
    );
    try {
      await Promise.all([local, readFacts(section, refresh, call)]);
    } catch {
      if (active && call === epoch)
        state.errors.set(section, "部分本机采集数据不可用，请查看各项状态。");
    }
  }
  async function save(patch: SettingsPatch, expected?: string) {
    const section: SettingsSection =
      patch.kind === "proxy"
        ? "network"
        : patch.kind === "display"
          ? "display"
          : "power";
    const snapshot = state.observations.get(section)?.snapshot;
    if (!snapshot) return;
    await run(section, {
      kind: "change",
      request: identity(),
      expected: expected ?? snapshot.revision,
      patch,
    });
  }
  async function keep(section: SettingsSection) {
    const response = state.observations.get(section);
    if (
      !port ||
      !response?.request ||
      !response.digest ||
      response.effect !== "preview"
    )
      return;
    try {
      const value = await port({
        kind: "keepDisplay",
        request: response.request,
        digest: response.digest,
      });
      if (active) accept(section, value);
    } catch (error) {
      if (active) state.errors.set(section, message(error));
    }
  }
  async function cancel(section: SettingsSection) {
    const pending = state.pending.get(section);
    if (!port || !pending) return;
    try {
      const response = await port({ kind: "cancel", request: pending.request });
      if (active) accept(section, response);
    } catch (error) {
      if (active) state.errors.set(section, message(error));
    }
  }
  async function recover(section: SettingsSection) {
    const pending = state.pending.get(section);
    if (!pending || state.busy.has(section)) return;
    await run(section, { kind: "status", request: pending.request });
  }
  async function retry(section: SettingsSection) {
    const pending = state.pending.get(section);
    if (!pending || state.busy.has(section)) return;
    await run(section, pending.operation);
  }
  const summary = computed(() => ({
    network: [...state.facts.values()].find((v) =>
      v.source.domains.includes("hardware.network"),
    )?.value,
    storage: [...state.facts.values()].find((v) =>
      v.source.domains.includes("hardware.volumes"),
    )?.value,
    power: [...state.facts.values()].find((v) =>
      v.source.domains.includes("hardware.battery"),
    )?.value,
  }));
  function dispose() {
    if (!active) return;
    // A disappeared interface cannot keep a preview. The worker owns the timeout and restore.
    for (const section of state.pending.keys()) void cancel(section);
    active = false;
    epoch++;
    clearInterval(timer);
    state.observations.clear();
    state.facts.clear();
    state.pending.clear();
  }
  return {
    state,
    available: !!port,
    sections,
    relevant,
    load,
    save,
    keep,
    cancel,
    recover,
    retry,
    summary,
    openNative: (section: SettingsSection) =>
      run(section, { kind: "openNative", section, request: identity() }),
    dispose,
  };
}
export type SystemSettingsController = ReturnType<typeof createSystemSettings>;
export const systemSettingsKey: InjectionKey<SystemSettingsController> =
  Symbol("system-settings");
