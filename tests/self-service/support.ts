import fixtures from "../assistant/execution-fixtures.json";
import type {
  BackendTask,
  ExecutionTaskDetails,
  Snapshot,
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
export function snapshot(): Snapshot {
  return {
    available: [offer()],
    preparations: [],
    selected: null,
    requests: [],
    next: null,
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
  | "recent_tasks"
  | "policy_directory"
  | "policy_invoke"
  | "policy_confirm"
  | "policy_status"
  | "observe_software"
> = {
  recent_tasks: async () => ({ items: [], next: null }),
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
