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
  TaskPhase,
  EffectAssessment,
  TaskControlReceipt,
  SoftwareState,
} from "../../apps/desktop/src/self-service/types";
import { categories } from "../../apps/desktop/src/software/presentation";
import {
  directoryItem,
  executionTask,
  policyRecord,
  taskDetail,
  taskFromFacts,
} from "../self-service/support";
/** Explicit UI samples. This is never imported by a production assembly. */
export function configurePageFixture(input: WorkspaceInput, scene = "normal") {
  const environment = input.environment!,
    local = environment.local,
    selfService = input.selfServicePort!;
  environment.appVersion = async () => "0.1.6-fixture";
  const readOnly = scene === "readonly",
    offline = scene === "offline",
    empty = scene === "empty",
    taskScene = scene.startsWith("tasks");
  const events: { operation: string; request?: string; controlId?: string }[] =
    [];
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
  let releaseDirectory!: () => void;
  const directoryRead =
    scene === "loading"
      ? new Promise<void>((resolve) => {
          releaseDirectory = resolve;
        })
      : Promise.resolve();
  selfService.policy_directory = async () => {
    await directoryRead;
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
  selfService.policy_invoke = async ({ choice, expected }) => {
    available();
    if (readOnly) throw Error("fixture denied");
    events.push({ operation: "policy_invoke", request: choice.request });
    if (records.has(choice.request))
      return deepClone(records.get(choice.request)!);
    const item = published.find((value) => value.policyId === choice.policyId);
    if (!item) throw Error("fixture missing");
    if (
      item.policyVersion !== expected.version ||
      item.policyRevision !== expected.revision
    )
      throw { code: "policy_selection_changed" };
    const record = policyRecord(choice.request);
    if (record.action.kind !== "policy") throw Error("fixture type");
    record.action.policyRevision = item.policyRevision;
    record.action.choice = deepClone(choice);
    record.action.task = deepClone(item.task);
    record.action.presentation = deepClone(item.selfService.presentation);
    record.action.definition = {
      id: item.policyId,
      revision: item.policyVersion,
    };
    record.state = "observed";
    if (taskScene && item.task.kind === "software") {
      record.action.offer = {
        task: `task-${choice.request}`,
        attempt: `attempt-${choice.request}`,
        request: choice.request,
        revision: "a".repeat(64),
        expiresAt: Math.floor(Date.now() / 1000) + 60,
        userInitiated: true,
        presentation: {
          title: String(item.selfService.presentation.displayName),
          summary: {
            kind: "software",
            intent: "install",
            steps: [
              {
                package: item.task.resource.id,
                version: item.policyId === "fixture-software-1" ? "2.0" : "1.0",
                identity: item.task.runAs === "system" ? "system" : "user",
              },
            ],
          },
        },
      };
    }
    records.set(choice.request, record);
    const detail = taskDetail(choice.request);
    detail.facts = { kind: "preparation", value: record };
    detail.taskKind = "policy";
    detail.policy = { id: choice.policyId, revision: item.policyVersion };
    detail.policyRevision = item.policyRevision;
    details.set(choice.request, detail);
    if (taskScene && item.task.kind === "software" && record.action.offer) {
      const detail = taskDetail(choice.request),
        offer = record.action.offer;
      detail.source = record.trigger.kind === "ai" ? "ai" : "human";
      detail.policy = deepClone(record.action.definition);
      detail.policyRevision = record.action.policyRevision;
      detail.taskKind = "software";
      if (
        detail.facts.kind === "execution" &&
        offer.presentation.summary.kind === "software"
      ) {
        detail.facts.value.action.initiator = {
          kind: "backend",
          task: offer.task,
          attempt: offer.attempt,
          trigger: deepClone(record.trigger),
        };
        detail.facts.value.action.actor = "fixture-system";
        detail.facts.value.action.policy = deepClone(record.action.definition);
        detail.facts.value.action.target = {
          device: "fixture-device",
          platform: "macos",
          scope: { kind: "device" },
        };
        detail.facts.value.action.runAs = { kind: "system", platform: "macos" };
        detail.facts.value.action.validity = {
          notBeforeUnixMs: Date.now(),
          expiresAtUnixMs: Date.now() + 60_000,
        };
        detail.facts.value.action.execution = {
          kind: "softwareProgram",
          intent: offer.presentation.summary.intent,
          steps: offer.presentation.summary.steps.map((step) => ({
            package: step.package,
            version: step.version,
            adapter: "pkg",
            deployment: null,
            runAs: { kind: "system", platform: "macos" },
          })),
        };
        detail.facts.value.action.operation.resource = deepClone({
          id: item.task.resource.id,
          revision: item.task.resource.version,
        });
        // Explicit detector fixture predates this invocation and is independent of its target.
        detail.softwareState =
          item.policyId === "fixture-software-1"
            ? { kind: "present", version: "1.0" }
            : { kind: "absent" };
        detail.facts.value.action.operation.action =
          offer.presentation.summary.intent;
        detail.facts.value.status.phase = "accepted";
        detail.facts.value.status.mode = "test";
      }
      detail.stage = "accepted";
      detail.recordedAt = Date.now();
      detail.nodes = [
        {
          id: `${choice.request}:accepted`,
          parent: null,
          stage: "accepted",
          attempt: null,
          bytes: null,
          total: null,
          reason: null,
          exitCode: null,
          detected: null,
          quiescent: null,
        },
      ];
      details.set(choice.request, detail);
    }
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
      const detail = taskDetail(choice.request);
      detail.stage = "waiting";
      detail.taskKind = "software";
      detail.recordedAt = Date.now();
      if (detail.facts.kind === "execution") {
        detail.facts.value.status = deepClone(status.execution);
        detail.facts.value.action.operation.action = choice.action;
        detail.facts.value.action.operation.resource = {
          id: choice.software.installation,
          revision: "fixture-observation",
        };
        detail.facts.value.action.initiator = deepClone(
          status.prepared.initiator,
        );
      }
      details.set(choice.request, detail);
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
      status.execution.phase = taskScene ? "accepted" : "executionEnded";
      status.execution.attempts = 1;
      const detail = details.get(request) ?? taskDetail(request);
      detail.stage = status.execution.phase;
      detail.active = true;
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
  const baseTask = local.tasks,
    controls = new Map<string, TaskControlReceipt>();
  function advanceTask(
    request: string,
    phase: TaskPhase,
    assessment: EffectAssessment | null = null,
    observation: SoftwareState | null = null,
  ) {
    const detail = details.get(request);
    if (!detail || detail.facts.kind !== "execution")
      throw Error("fixture execution missing");
    const status = detail.facts.value.status;
    status.phase = phase;
    status.assessment = assessment;
    detail.stage = phase;
    detail.active = [
      "waiting",
      "confirmationRequired",
      "accepted",
      "running",
      "executionEnded",
      "outcomeUnknown",
    ].includes(phase);
    detail.allowedActions = detail.active ? ["cancel"] : [];
    detail.facts.value.revision++;
    detail.revision = { owner: detail.facts.value.revision, transfers: [] };
    if (phase === "executionEnded" || phase === "verified")
      status.process = {
        finished: true,
        exitCode: 0,
        end: "exited",
        failureKind: "none",
        quiescent: true,
        totalOutputBytes: 0,
        quality: "complete",
      };
    if (phase === "cancelled")
      status.process = {
        finished: true,
        exitCode: null,
        end: "cancelled",
        failureKind: "none",
        quiescent: true,
        totalOutputBytes: 0,
        quality: "complete",
      };
    if (observation) detail.softwareState = deepClone(observation);
    detail.nodes.push({
      id: `${request}:${phase}:${detail.nodes.length}`,
      parent: null,
      stage: phase,
      bytes: null,
      total: null,
      attempt: status.attemptId,
      reason: null,
      exitCode: status.process?.exitCode ?? null,
      detected: deepClone(detail.softwareState),
      quiescent: status.process?.quiescent ?? null,
    });
    const software = operations.get(request);
    if (software) {
      software.execution = deepClone(status);
      software.detected = deepClone(detail.softwareState);
    }
    return deepClone(detail);
  }
  if (taskScene) {
    for (const [request, source, phase, assessment] of [
      ["fixture-install", "human", "running", null],
      ["fixture-update", "human", "executionEnded", null],
      ["fixture-ai-unknown", "ai", "outcomeUnknown", null],
      ["fixture-verified", "human", "verified", "satisfied"],
      ["fixture-not-satisfied", "human", "verified", "notSatisfied"],
    ] as const) {
      const detail = taskDetail(request);
      detail.source = source;
      detail.taskKind = "software";
      detail.recordedAt = Date.now();
      if (detail.facts.kind === "execution") {
        detail.facts.value.action.initiator = {
          kind: "human",
          osSession: {
            device: "fixture-device",
            account: { platform: "macos", subject: "501" },
            session: "fixture-session",
          },
        };
        detail.facts.value.action.actor = "fixture-system";
        detail.facts.value.action.target = {
          device: "fixture-device",
          platform: "macos",
          scope: { kind: "device" },
        };
        detail.facts.value.action.runAs = { kind: "system", platform: "macos" };
        detail.facts.value.action.validity = {
          notBeforeUnixMs: Date.now(),
          expiresAtUnixMs: Date.now() + 60_000,
        };
        detail.facts.value.status.mode = "test";
        detail.facts.value.action.operation.action =
          request === "fixture-update" ? "update" : "install";
        detail.facts.value.action.operation.resource.id =
          request === "fixture-update"
            ? "Work Notes"
            : request === "fixture-ai-unknown"
              ? "AI Work Tool"
              : request === "fixture-not-satisfied"
                ? "Unmet Application"
                : "Workspace";
        if (source === "ai")
          detail.facts.value.action.initiator = {
            kind: "ai",
            provider: "codex",
            config: { id: "cfg", revision: "1" },
            conversation: "fixture-conversation",
            toolCall: "fixture-tool-call",
            osSession: {
              device: "fixture-device",
              account: { platform: "macos", subject: "501" },
              session: "fixture-session",
            },
          };
      }
      details.set(request, detail);
      advanceTask(
        request,
        phase,
        assessment,
        phase === "verified"
          ? assessment === "satisfied"
            ? { kind: "present", version: "1.0" }
            : { kind: "absent" }
          : null,
      );
    }
    const management = taskDetail("management-original");
    management.reference = { kind: "activity", id: "fixture-policy-activity" };
    management.source = "policy";
    management.taskKind = "management";
    management.stage = "changed";
    management.active = false;
    management.allowedActions = [];
    management.facts = {
      kind: "management",
      value: {
        id: "fixture-policy-activity",
        source: "fixture-policy",
        kind: "configuration",
        phase: "changed",
        recordedAt: Math.floor(Date.now() / 1000),
        observedAt: Math.floor(Date.now() / 1000),
        origin: "policy",
        target: null,
        links: {
          policy: "policy-original",
          version: "v1",
          batch: null,
          member: null,
          request: null,
          operation: null,
          retirement: null,
          task: null,
          attempt: null,
          steps: [],
          operations: [],
        },
        delivery: null,
        execution: null,
        effect: null,
        reporting: null,
        decision: null,
        auditOutcome: "recorded",
      },
    };
    details.set("management-original", management);
  }
  const visibleTask = (task: TaskDetail) => {
    const value = deepClone(task);
    if (readOnly) {
      value.allowedActions = [];
      value.humanConfirmation = false;
      value.controlReason = "当前用户只能读取原任务";
    }
    return value;
  };
  if (baseTask)
    local.tasks = {
      ...baseTask,
      detail: async (reference) => {
        available();
        if (scene === "tasks-denied") throw { code: "task_denied" };
        const value = [...details.values()].find(
          (item) =>
            JSON.stringify(item.reference) === JSON.stringify(reference),
        );
        if (!value) return visibleTask(await baseTask.detail(reference));
        if (taskScope) value.scope = taskScope;
        events.push({
          operation: "task_detail",
          request: reference.kind === "request" ? reference.request : undefined,
        });
        return visibleTask(value);
      },
      list: async (query) => {
        available();
        const page = await baseTask.list(query);
        taskScope = page.scope;
        for (const detail of details.values()) detail.scope = taskScope;
        const items = (
          empty
            ? []
            : [...details.values()]
                .map((value) => deepClone(value))
                .concat(taskScene ? [] : page.items)
                .filter(
                  (value) =>
                    (!query.policy || value.policy?.id === query.policy) &&
                    (!query.source || value.source === query.source) &&
                    (query.view === "management"
                      ? value.facts.kind === "management"
                      : query.view === "active"
                        ? value.active && value.facts.kind !== "management"
                        : value.facts.kind !== "management"),
                )
        ).map(visibleTask);
        return {
          ...page,
          items,
          asOf: Date.now(),
          remoteAvailable: taskScene,
          coverage: taskScene
            ? [
                { source: "human", complete: true, reasons: [] },
                { source: "ai", complete: true, reasons: [] },
                {
                  source: "policy",
                  complete: scene !== "tasks-partial",
                  reasons:
                    scene === "tasks-partial" ? ["当前管理来源仅部分覆盖"] : [],
                },
              ]
            : page.coverage,
          complete: scene !== "tasks-partial",
        };
      },
      control: async (input) => {
        available();
        if (readOnly) throw { code: "task_denied" };
        const existing = controls.get(input.id);
        if (existing) return deepClone(existing);
        const task = [...details.values()].find(
          (item) =>
            JSON.stringify(item.reference) === JSON.stringify(input.reference),
        );
        if (!task && !taskScene) return baseTask.control(input);
        if (
          !task ||
          task.scope !== input.scope ||
          JSON.stringify(task.revision) !==
            JSON.stringify(input.expectedRevision) ||
          !task.allowedActions.includes(input.action)
        )
          throw { code: "task_stale" };
        events.push({
          operation: `task_${input.action}`,
          request:
            task.reference.kind === "request"
              ? task.reference.request
              : undefined,
          controlId: input.id,
        });
        if (input.action === "cancel" && task.facts.kind === "execution") {
          task.facts.value.status.cancelRequested = true;
          task.facts.value.status.stopOutcome = "acknowledged";
          task.facts.value.revision++;
          task.revision = { owner: task.facts.value.revision, transfers: [] };
        }
        const receipt: TaskControlReceipt = {
          id: input.id,
          disposition:
            scene === "tasks-control-unknown" ? "unknown" : "confirmed",
          created: null,
        };
        controls.set(input.id, receipt);
        return deepClone(receipt);
      },
      controlStatus: async (input) => {
        available();
        events.push({ operation: "task_control_status", controlId: input.id });
        const receipt = controls.get(input.id);
        if (!receipt)
          return { id: input.id, disposition: "unknown", created: null };
        receipt.disposition = "confirmed";
        return deepClone(receipt);
      },
    };
  const snapshots = new Map<string, SettingsResponse>(),
    settingValues = new Map<string, unknown>(),
    settingRevisions = new Map<string, number>(),
    previewsBefore = new Map<string, SettingsResponse>();
  const advanceRevision = (section: string) =>
    settingRevisions.set(section, (settingRevisions.get(section) ?? 1) + 1);
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
    for (const field of target.fields) {
      const key = `${section}:${field.key}`;
      if (!settingValues.has(key))
        settingValues.set(key, deepClone(field.value));
      field.value = deepClone(settingValues.get(key)) as typeof field.value;
    }
    return {
      request: null,
      digest: null,
      finished: true,
      effect: "observed",
      writeAllowed: !readOnly,
      snapshot: {
        section,
        revision: (settingRevisions.get(section) ?? 1)
          .toString(16)
          .padStart(64, "0"),
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
      if (response.effect === "preview") {
        if (Date.now() >= (response.keepBeforeUnixMs ?? 0)) {
          const before = previewsBefore.get(operation.request)!;
          for (const field of before.snapshot!.targets[0].fields)
            settingValues.set(`display:${field.key}`, deepClone(field.value));
          advanceRevision("display");
          response.snapshot = observed("display").snapshot;
          response.effect = "restored";
          response.finished = true;
        } else if (operation.kind === "keepDisplay") {
          if (operation.digest !== response.digest)
            throw Error("fixture digest conflict");
          response.effect = "applied";
          response.finished = true;
        }
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
    const previous = snapshots.get(operation.request);
    if (previous) return deepClone(previous);
    const before = observed(section);
    if (operation.kind === "change") {
      if (operation.expected !== before.snapshot?.revision)
        throw Error("fixture revision conflict");
      if (operation.patch.target !== before.snapshot.targets[0]?.id)
        throw Error("fixture target missing");
      for (const [key, value] of Object.entries(operation.patch))
        if (!["kind", "target"].includes(key) && value !== null)
          settingValues.set(`${section}:${key}`, deepClone(value));
      advanceRevision(section);
    }
    const response = observed(section);
    response.request = operation.request;
    response.digest = "a".repeat(64);
    response.effect =
      operation.kind === "openNative" ? "openRequested" : "applied";
    events.push({
      operation: `settings_${operation.kind}`,
      request: operation.request,
    });
    if (operation.kind === "change" && section === "display") {
      response.effect = "preview";
      response.finished = false;
      response.keepBeforeUnixMs = Date.now() + 15000;
      previewsBefore.set(operation.request, before);
    }
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
  // Explicit development seam: real client deliveries bind these records through the production trust adapter.
  const assistant = input.assistantServices!;
  const readAssistantTask = assistant.taskDetails!;
  assistant.taskDetails = async (request, signal) => {
    if (signal.aborted) throw Error("fixture aborted");
    return records.has(request)
      ? deepClone(
          taskFromFacts({ kind: "preparation", value: records.get(request)! }),
        )
      : readAssistantTask(request, signal);
  };
  assistant.preparationDetails = async (request) => {
    const record = records.get(request);
    if (!record) throw Error("fixture missing operation");
    return deepClone(record);
  };
  assistant.submitOperation = async ({ request, revision, parameters }) => {
    const record = records.get(request);
    if (
      !record ||
      record.state !== "draft" ||
      record.revision !== revision ||
      record.action.kind !== "policy"
    )
      throw { code: "operation_conflict" };
    if (
      typeof parameters.name !== "string" ||
      !parameters.name.trim() ||
      parameters.name.length > 32
    )
      throw { code: "operation_invalid_parameters" };
    events.push({ operation: "ai_operation_submit", request });
    record.action.choice.parameters = deepClone(parameters);
    record.state = "submitting";
    record.revision++;
    details.set(request, taskFromFacts({ kind: "preparation", value: record }));
  };
  assistant.reconcilePolicy = async () => {};
  const stageAssistantOperation = (sessionId: string, operationId: string) => {
    const record = policyRecord(operationId);
    record.state = "draft";
    record.trigger = {
      kind: "ai",
      conversation: sessionId,
      toolCall: operationId,
      context: { id: "fixture-binding", generation: "fixture-generation" },
      config: { id: "fixture", revision: "1" },
      osSession: {
        device: "fixture-device",
        session: "fixture-os-session",
        account: { platform: "macos", subject: "fixture-account" },
      },
    };
    if (record.action.kind !== "policy") throw Error("fixture type");
    record.action.presentation = {
      displayName: "核对组织修复参数",
      description: "只提交本次样本准备，不执行系统操作",
      sideEffects: "仅改变隔离样本记录",
    };
    record.action.choice.parameters = {
      name: "AI 建议",
      path: "/Applications",
    };
    record.action.task = {
      kind: "script",
      runAs: "system",
      resource: { id: "repair-fixture", version: "1", variant: "macos" },
      parameterSources: {
        name: { kind: "input" },
        path: { kind: "fixed", value: "/Applications" },
      },
      parameterSchema: {
        type: "object",
        properties: {
          name: { type: "string", title: "修复名称", maxLength: 32 },
          path: { type: "string", title: "固定路径" },
        },
        required: ["name", "path"],
      },
    };
    records.set(operationId, record);
    details.set(
      operationId,
      taskFromFacts({ kind: "preparation", value: record }),
    );
  };
  return {
    events,
    releaseDirectory: () => releaseDirectory?.(),
    published,
    operations,
    records,
    details,
    advanceTask,
    stageAssistantOperation,
  };
}
