import fixtures from "../assistant/execution-fixtures.json";
import type {
  BackendTask,
  ExecutionTaskDetails,
  SelfServicePort,
} from "../../apps/desktop/src/self-service/types";
export function executionTask(): ExecutionTaskDetails {
  return structuredClone(fixtures.running) as ExecutionTaskDetails;
}
export function offer(): BackendTask {
  return {
    task: "backend-task",
    attempt: "backend-attempt",
    request: "backend-request",
    revision: "a".repeat(64),
    presentation: {
      title: "固定软件 1.0",
      summary: {
        kind: "software",
        intent: "install",
        steps: [{ package: "固定软件", version: "1.0", identity: "system" }],
      },
    },

    expiresAt: 9999999999,
    userInitiated: true,
  };
}
/** The same public backend projection consumed by the desktop, with explicit test assembly. */
export function resourceOffer(): BackendTask {
  return {
    ...offer(),
    presentation: {
      title: "办公套件",
      summary: {
        kind: "software",
        intent: "install",
        steps: [{ package: "办公套件", version: "1.0", identity: "system" }],
      },
    },
  };
}

/** Empty formal directory is explicit fixture data, never a production fallback. */
export const policyFixturePort: Pick<
  SelfServicePort,
  | "offers"
  | "policy_resolve"
  | "policy_directory"
  | "policy_invoke"
  | "policy_confirm"
  | "policy_status"
  | "observe_software"
> = {
  offers: async () => [],
  policy_resolve: async () => {
    throw new Error("fixture policy not configured");
  },
  policy_directory: async () => ({ items: [], nextCursor: null, asOf: 1000 }),
  policy_invoke: async () => {
    throw new Error("fixture policy not configured");
  },
  policy_confirm: async () => {
    throw new Error("fixture policy not configured");
  },
  policy_status: async () => {
    throw new Error("fixture policy not configured");
  },
  observe_software: async () => {
    throw new Error("fixture detector not configured");
  },
};

/** A minimal authorized script request for UI request recovery journeys. */
export function policyRecord(
  request = "original-policy",
): import("../../apps/desktop/src/self-service/types").BackendRequest {
  return {
    recordedAtUnixMs: 1000,
    cancellationControl: null,
    retryOf: null,
    action: {
      kind: "policy",
      definition: { id: "policy", revision: "version" },
      policyRevision: 1,
      choice: {
        request,
        policyId: "policy",
        parameters: {},
      },

      task: {
        kind: "script",
        runAs: "system",
        resource: { id: "r", version: "1", variant: "macos" },
        parameterSchema: { type: "object" },
        parameterSources: {},
      },
      presentation: {
        displayName: "Published tool",
        description: "Read state",
      },
      access: { kind: "device" },
      expiresAtUnixMs: Date.now() + 60_000,
      runId: null,
      offer: null,
    },
    trigger: {
      kind: "human",
      osSession: {
        device: "d",
        account: { platform: "macos", subject: "501" },
        session: "login",
      },
    },
    risk: null,
    confirmation: null,
    revision: 1,
    state: "awaitingConfirmation",
    failure: null,
  };
}

export function directoryItem(
  action: import("../../apps/desktop/src/self-service/types").PreparedAction,
): import("../../apps/desktop/src/self-service/types").SelfServiceDirectoryItem {
  if (action.kind !== "policy") throw Error("policy fixture");
  const id = action.choice.policyId;
  return {
    policyId: id,
    policyRevision: 1,
    policyVersion: "version",
    subject: { kind: "device" },
    capability: {
      id,
      objectType: "policy",
      toolName: `policy_${id.replaceAll("-", "")}`,
      alias: null,
      scope: "scope",
      instance: "instance",
      tenant: "tenant",
      revision: 1,
      version: "version",
      operations: ["invoke"],
      inputSchema: {},
      outputSchema: {},
    },
    task: action.task,
    selfService: {
      published: true,
      allowAi: true,
      riskLevel: 1,
      presentation: action.presentation,
    },
  };
}

export function taskDetail(
  request = "original-request",
): import("../../apps/desktop/src/self-service/types").TaskDetail {
  const facts = executionTask();
  facts.action.requestId = request;
  facts.status.operationRequestId = request;
  return {
    reference: { kind: "request", request },
    scope: "native-scope",
    revision: { owner: facts.revision, transfers: [] },
    recordedAt: 1000,
    source: "human",
    taskKind: "policy",
    stage: facts.status.phase,
    active: true,
    facts: { kind: "execution", value: facts },
    nodes: [],
    allowedActions: ["cancel"],
    controlReason: null,
    uploadAcknowledged: null,
    collectionQuality: null,
    collectionComplete: null,
    softwareState: null,
    policy: null,
    policyRevision: null,
    retryOf: null,
    related: [],
    missing: [],
    parameterSummary: {},
    humanConfirmation: false,
  };
}
export function taskPage(
  items: import("../../apps/desktop/src/self-service/types").TaskDetail[] = [],
): import("../../apps/desktop/src/self-service/types").TaskPage {
  return {
    items,
    next: null,
    scope: "native-scope",
    asOf: 1000,
    lastSync: null,
    remoteAvailable: false,
    coverage: [],
    retainedFrom: null,
    complete: true,
  };
}
export function taskPort(): import("../../apps/desktop/src/self-service/types").TaskPort {
  return {
    list: async () => taskPage(),
    detail: async (ref) => {
      if (ref.kind !== "request") throw Error("fixture missing");
      return taskDetail(ref.request);
    },
    control: async (input) => ({
      id: input.id,
      disposition: "confirmed",
      created: null,
    }),
    controlStatus: async (input) => ({
      id: input.id,
      disposition: "unknown",
      created: null,
    }),
  };
}
