import { deepClone } from "valtio/vanilla/utils";
import type { WorkspaceInput } from "../../apps/desktop/src/state/workspace";
import type {
  BackendRequest,
  LocalSoftwareStatus,
  NetworkCheckStatus,
  PersonalVaultStatus,
  QueryStatus,
  SelfServiceDirectoryItem,
  SettingsResponse,
  TaskDetail,
} from "../../apps/desktop/src/self-service/types";
import { categories } from "../../apps/desktop/src/software/presentation";
import {
  directoryItem,
  executionTask,
  policyRecord,
  taskDetail,
} from "../self-service/support";
/** Explicit UI samples. This is never imported by a production assembly. */
export function configurePageFixture(input: WorkspaceInput, scene = "normal") {
  const environment = input.environment!,
    local = environment.local,
    selfService = input.selfServicePort!;
  environment.appVersion = async () => "0.1.6-fixture";
  const readOnly = scene === "readonly",
    offline = scene === "offline",
    empty = scene === "empty";
  const events: { operation: string; request?: string }[] = [];
  const operations = new Map<string, LocalSoftwareStatus>(),
    records = new Map<string, BackendRequest>(),
    queries = new Map<string, QueryStatus>(),
    details = new Map<string, TaskDetail>();
  const published: SelfServiceDirectoryItem[] = categories.map(
    (category, index) => {
      const record = policyRecord(),
        item = directoryItem(record.action);
      item.policyId = `fixture-software-${index}`;
      item.capability.id = item.policyId;
      item.task = {
        kind: "software",
        resource: {
          id: `fixture-package-${index}`,
          version: "definition-1",
          variant: "macos",
        },
        runAs: "system",
        target: { kind: "device" },
      };
      item.selfService.presentation = {
        displayName: [
          "工作空间",
          "创作画板",
          "网络浏览器",
          "开发笔记",
          "设备同步",
        ][index],
        publisher: "Fixture Studio",
        category,
        description: `用于 ${category} 的显式样本发布项目；不代表本机已安装。`,
      };
      return item;
    },
  );
  const script = directoryItem(policyRecord().action);
  script.policyId = "fixture-tool";
  script.capability.id = script.policyId;
  script.selfService.presentation = {
    displayName: "组织网络观察",
    description: "组织发布的显式样本工具，填写参数后手动提交。",
  };
  script.task = {
    kind: "script",
    resource: {
      id: "fixture-script",
      version: "definition-1",
      variant: "macos",
    },
    runAs: "system",
    parameterSources: {
      interface: { kind: "input" },
      verbose: { kind: "input" },
    },
    parameterSchema: {
      type: "object",
      properties: {
        interface: { type: "string", title: "网络接口" },
        verbose: { type: "boolean", title: "详细观察" },
      },
      required: ["interface"],
    },
  };
  published.push(script);
  const available = () => {
    if (offline || scene === "error") throw Error("fixture unavailable");
  };
  selfService.policy_directory = async () => {
    available();
    return {
      items: empty ? [] : deepClone(published),
      nextCursor: null,
      asOf: Math.floor(Date.now() / 1000),
    };
  };
  selfService.policy_resolve = async (selector) => {
    available();
    if (selector.kind !== "id") throw Error("fixture selector");
    const item = published.find((value) => value.policyId === selector.id);
    if (!item) throw Error("fixture missing");
    return deepClone(item);
  };
  selfService.policy_invoke = async ({ choice }) => {
    available();
    if (readOnly) throw Error("fixture denied");
    events.push({ operation: "policy_invoke", request: choice.request });
    if (records.has(choice.request))
      return deepClone(records.get(choice.request)!);
    const item = published.find((value) => value.policyId === choice.policyId);
    if (!item) throw Error("fixture missing");
    const record = policyRecord(choice.request);
    if (record.action.kind !== "policy") throw Error("fixture type");
    record.action.choice = deepClone(choice);
    record.action.task = deepClone(item.task);
    record.action.presentation = deepClone(item.selfService.presentation);
    record.action.definition = {
      id: item.policyId,
      revision: item.policyVersion,
    };
    record.state = "observed";
    records.set(choice.request, record);
    const detail = taskDetail(choice.request);
    detail.facts = { kind: "preparation", value: record };
    detail.taskKind = "policy";
    detail.policy = { id: choice.policyId, revision: item.policyVersion };
    detail.policyRevision = item.policyRevision;
    details.set(choice.request, detail);
    return deepClone(record);
  };
  selfService.policy_status = async ({ requestId }) => {
    available();
    const record = records.get(requestId);
    return record
      ? { kind: "preparation", value: deepClone(record) }
      : { kind: "absent" };
  };
  if (local.software) {
    const read = local.software.read;
    local.software.read = async (request) => {
      available();
      const page = await read(request);
      return {
        ...page,
        items: empty
          ? []
          : page.items.map((item, index) => ({
              ...item,
              actions: [
                {
                  action: "launch",
                  available: !readOnly,
                  reason: readOnly ? "ruleDisabled" : null,
                  targetVersion: null,
                },
                {
                  action: "uninstall",
                  available: !readOnly && index !== 0,
                  reason:
                    index === 0
                      ? "protected"
                      : readOnly
                        ? "ruleDisabled"
                        : null,
                  targetVersion: null,
                },
                {
                  action: "update",
                  available: !readOnly && index === 1,
                  reason: readOnly
                    ? "ruleDisabled"
                    : index === 1
                      ? null
                      : "updateUnknown",
                  targetVersion: index === 1 ? "2.2" : null,
                },
              ],
            })),
      };
    };
    local.software.prepare = async (choice) => {
      available();
      if (readOnly) throw Error("fixture denied");
      events.push({ operation: "software_prepare", request: choice.request });
      const status: LocalSoftwareStatus = {
        detected: null,
        prepared: {
          request: choice.request,
          digest: "a".repeat(64),
          choice: deepClone(choice),
          name: choice.software.installation,
          initiator: {
            kind: "human",
            osSession: {
              device: "fixture",
              account: { platform: "macos", subject: "501" },
              session: "fixture",
            },
          },
          identity: "system",
          targetVersion: choice.action === "update" ? "2.2" : null,
          startBeforeUnixMs: Date.now() + 60000,
        },
        execution: {
          ...executionTask().status,
          operationRequestId: choice.request,
          contentDigest: "a".repeat(64),
          phase: "waiting",
          mode: "test",
          attempts: 0,
          attemptId: null,
          admission: null,
          cancelRequested: false,
        },
      };
      operations.set(choice.request, status);
      return deepClone(status.prepared);
    };
    local.software.status = async ({ request }) => {
      available();
      const status = operations.get(request);
      if (!status) throw Error("fixture missing");
      return deepClone(status);
    };
    local.software.submit = async ({ request, digest }) => {
      available();
      const status = operations.get(request);
      if (!status || status.prepared.digest !== digest)
        throw Error("fixture mismatch");
      events.push({ operation: "software_submit", request });
      status.execution.phase = "executionEnded";
      status.execution.attempts = 1;
      const detail = taskDetail(request);
      detail.stage = "executionEnded";
      detail.active = false;
      detail.taskKind = "software";
      if (detail.facts.kind === "execution")
        detail.facts.value.status = deepClone(status.execution);
      details.set(request, detail);
      return deepClone(status);
    };
    local.software.history = async () => ({
      items: [...operations.values()].map((value) => deepClone(value)),
      next: null,
    });
  }
  if (local.data) {
    local.data.query_submit = async ({ request, query }) => {
      available();
      events.push({ operation: "query_submit", request });
      const status: QueryStatus = {
        request,
        query,
        phase: "completed",
        cancelRequested: false,
        preparation: null,
      };
      queries.set(request, status);
      return deepClone(status);
    };
    local.data.query_status = async ({ request }) => {
      available();
      const value = queries.get(request);
      if (!value) throw Error("fixture missing");
      return deepClone(value);
    };
  }
  let taskScope: string | undefined;
  const baseTask = local.tasks;
  if (baseTask)
    local.tasks = {
      ...baseTask,
      detail: async (reference) => {
        const value =
          reference.kind === "request"
            ? details.get(reference.request)
            : undefined;
        if (!value) return baseTask.detail(reference);
        if (taskScope) value.scope = taskScope;
        return deepClone(value);
      },
      list: async (query) => {
        const page = await baseTask.list(query);
        taskScope = page.scope;
        for (const detail of details.values()) detail.scope = taskScope;
        return {
          ...page,
          items: [...details.values()]
            .map((value) => deepClone(value))
            .concat(page.items)
            .filter(
              (value) => !query.policy || value.policy?.id === query.policy,
            ),
        };
      },
    };
  const snapshots = new Map<string, SettingsResponse>();
  const observed = (
    section: import("../../apps/desktop/src/self-service/types").SettingsSection,
  ): SettingsResponse => {
    const target =
      section === "network"
        ? {
            id: "wifi",
            name: "Wi-Fi",
            fields: [
              {
                key: "http",
                writable: !readOnly,
                value: { enabled: false, host: "", port: 0 },
                quality: "complete" as const,
                reason: null,
              },
            ],
            options: [],
            reason: readOnly ? "此用户仅可读取" : null,
          }
        : section === "display"
          ? {
              id: "internal",
              name: "内建显示器",
              fields: [
                {
                  key: "mode",
                  writable: !readOnly,
                  value: "default",
                  quality: "complete" as const,
                  reason: null,
                },
              ],
              options: [
                { id: "default", label: "默认 · 1512 × 982" },
                { id: "large", label: "较大文字 · 1280 × 800" },
              ],
              reason: null,
            }
          : section === "power"
            ? {
                id: "ac",
                name: "外接电源",
                fields: [
                  {
                    key: "displayOffMinutes",
                    writable: !readOnly,
                    value: 10,
                    quality: "complete" as const,
                    reason: null,
                  },
                  {
                    key: "sleepMinutes",
                    writable: !readOnly,
                    value: 30,
                    quality: "complete" as const,
                    reason: null,
                  },
                ],
                options: [],
                reason: null,
              }
            : {
                id: section,
                name:
                  section === "sound"
                    ? "内建音频"
                    : section === "security"
                      ? "设备保护"
                      : "系统观察",
                fields: [
                  {
                    key: section === "sound" ? "output" : "facts",
                    writable: false,
                    value: section === "sound" ? "内建扬声器" : "显式样本观察",
                    quality: "complete" as const,
                    reason: "此项只读；可打开系统设置",
                  },
                ],
                options: [],
                reason: null,
              };
    return {
      request: null,
      digest: null,
      finished: true,
      effect: "observed",
      writeAllowed: !readOnly,
      snapshot: {
        section,
        revision: "1".repeat(64),
        collectedAtUnixMs: Date.now(),
        source: "显式 UI 样本",
        targets: empty ? [] : [target],
      },
      reason: null,
      keepBeforeUnixMs: null,
    };
  };
  local.settings = async (operation) => {
    available();
    if (operation.kind === "snapshot") return observed(operation.section);
    if (operation.kind === "status" || operation.kind === "keepDisplay") {
      const response = snapshots.get(operation.request);
      if (!response) throw Error("fixture missing");
      if (operation.kind === "keepDisplay") {
        response.effect = "applied";
        response.finished = true;
      }
      return deepClone(response);
    }
    if (operation.kind === "change" && readOnly) throw Error("fixture denied");
    const section =
      operation.kind === "change"
        ? operation.patch.kind === "proxy"
          ? "network"
          : operation.patch.kind === "display"
            ? "display"
            : "power"
        : operation.section;
    const response = observed(section);
    response.request = operation.request;
    response.digest = "a".repeat(64);
    response.effect =
      operation.kind === "openNative" ? "openRequested" : "applied";
    events.push({
      operation: `settings_${operation.kind}`,
      request: operation.request,
    });
    snapshots.set(operation.request, response);
    return deepClone(response);
  };
  let network: NetworkCheckStatus | null = null;
  local.network = readOnly
    ? null
    : async (operation) => {
        available();
        if (operation.kind === "defaults")
          return {
            kind: "defaults",
            value: { host: "service.fixture.invalid", port: 443 },
          };
        if (operation.kind === "run") {
          events.push({ operation: "network_run", request: operation.request });
          network = {
            status: {
              ...executionTask().status,
              operationRequestId: operation.request,
              phase: "executionEnded",
              mode: "test",
            },
            result: {
              target: deepClone(operation.target),
              collectedAtUnixMs: Date.now(),
              dns: { address: null, finding: "reachable", durationMs: 4 },
              tcp: [
                {
                  address: "192.0.2.24:443",
                  finding: "reachable",
                  durationMs: 8,
                },
              ],
            },
          };
        }
        if (!network || network.status.operationRequestId !== operation.request)
          throw Error("fixture missing");
        return { kind: "status", value: deepClone(network) };
      };
  let vault: PersonalVaultStatus = {
    initialized: true,
    unlocked: false,
    passwordEnabled: true,
    revision: 1,
    entries: [
      {
        credentialId: [1, 2, 3],
        userHandle: [4, 5, 6],
        rpId: "work.fixture.invalid",
        username: "studio",
        displayName: "Studio User",
        backupEligible: true,
        backedUp: false,
      },
    ],
    provider: { enabled: null, indexPending: true },
  };
  environment.personalVault = async (operation) => {
    available();
    if (operation.kind !== "status") {
      if (readOnly) throw Error("fixture denied");
      events.push({ operation: `vault_${operation.kind}` });
    }
    if (operation.kind === "unlock") vault.unlocked = true;
    if (operation.kind === "lock") vault.unlocked = false;
    if (operation.kind === "delete") {
      if (operation.revision !== vault.revision)
        throw Error("fixture conflict");
      vault.entries = vault.entries.filter(
        (entry) =>
          JSON.stringify(entry.credentialId) !==
          JSON.stringify(operation.credential_id),
      );
      vault.revision++;
    }
    if (operation.kind === "reset")
      vault = {
        initialized: false,
        unlocked: false,
        passwordEnabled: false,
        revision: vault.revision + 1,
        entries: [],
        provider: null,
      };
    return deepClone(vault);
  };
  if (empty) vault.entries = [];
  if (offline || scene === "error") {
    input.assistantServices!.connect = async () => {
      throw Error("fixture AI unavailable");
    };
    environment.host.read = async () => {
      throw Error("fixture host unavailable");
    };
    environment.service.read = async () => {
      throw Error("fixture service unavailable");
    };
  }
  if (readOnly) {
    environment.personalVault = undefined;
    environment.connectionSave = undefined;
  }
  const previews = new Set<string>();
  environment.diagnostics = {
    prepare: async (scopes) => {
      available();
      const snapshot = crypto.randomUUID();
      previews.add(snapshot);
      events.push({ operation: "diagnostics_prepare", request: snapshot });
      return {
        snapshot,
        bytes: 128,
        content: {
          mode: "explicit-ui-fixture",
          scopes,
          service: "connected",
          credentials: "redacted",
        },
      };
    },
    export: async (snapshot) => {
      available();
      if (!previews.has(snapshot)) throw Error("fixture expired");
      events.push({ operation: "diagnostics_export", request: snapshot });
      return false;
    },
    discard: async (snapshot) => {
      previews.delete(snapshot);
      events.push({ operation: "diagnostics_discard", request: snapshot });
    },
  };
  return { events, published, operations, records };
}
