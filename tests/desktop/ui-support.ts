import { vi } from "vitest";
import type { UserContext } from "../../packages/ai-contract/dist/wire";
import type {
  RuntimeClient,
  SessionView,
} from "../../packages/ai-client/dist/index";
import { fixtureSession } from "../../packages/ai-contract/dist/testing";
import { activeStage } from "../../packages/ai-contract/dist/index";
import { createLoginFixture } from "../assistant/login-fixture";
import { fixtureLocalPorts } from "../assistant/local-ports";
import {
  policyFixturePort,
  taskPage,
  taskDetail,
} from "../self-service/support";
import type { WorkspaceInput } from "../../apps/desktop/src/state/workspace";
export function shellFixture() {
  let counter = 0;
  const guest = (): UserContext => ({
    schemaVersion: 1,
    kind: "userContext",
    generation: `device-${++counter}`,
    user: {
      schemaVersion: 1,
      kind: "testUser",
      userId: "device",
      displayName: "本机工作区",
      nameKey: "device",
    },
    identity: {
      mode: "device",
      authorityId: "fixture-device",
      tenantId: "local",
      principalId: "device",
    },
  });
  let identity = guest();
  const clients: RuntimeClient[] = [];
  const session = fixtureSession(),
    view: SessionView = {
      namespace: session.namespace,
      generation: activeStage(session).binding.generation,
      cursor: 0,
      capabilities: activeStage(session).capabilities,
      sessionStatus: "active",
      connection: "attached",
      timeline: [],
      commands: {},
      messages: {},
      interactions: {},
      surfaces: {},
      tools: {},
      deliveries: {},
    };
  const submit = vi.fn(async () => ({ kind: "receipt" })),
    execute = vi.fn(),
    querySubmit = vi.fn(),
    readData = fixtureLocalPorts();
  if (readData.data) {
    querySubmit.mockImplementation(readData.data.query_submit);
    readData.data.query_submit = querySubmit;
  }
  const recent = taskDetail();
  recent.stage = "executionEnded";
  recent.active = false;
  recent.recordedAt = Date.now();
  if (recent.facts.kind === "execution")
    recent.facts.value.status.phase = "executionEnded";
  const list = vi.fn(async () => taskPage([recent]));
  const input: WorkspaceInput = {
    environment: {
      kind: "fixture",
      identity: {
        read: async () => structuredClone(identity),
        guest: async () => {
          identity = guest();
          return structuredClone(identity);
        },
      },
      device: {
        read: vi.fn(async () => ({
          attributes: {
            computerName: "Current Device",
            loginUser: "os-user",
            ipAddresses: ["192.0.2.1"],
          },
        })),
      },
      host: {
        read: async () => ({
          schemaVersion: 1,
          kind: "hostStatus",
          generation: 1,
          phase: "ready",
          source: "development_override",
          version: "fixture",
          recent: [],
        }),
        restart: async () => {
          throw Error("unused");
        },
      },
      local: {
        ...readData,
        tasks: {
          list,
          detail: vi.fn(async () => taskDetail()),
          control: vi.fn(),
          controlStatus: vi.fn(),
        },
      },
      service: {
        read: async () => ({ phase: "unavailable" }),
        registration: async () => ({
          phase: "unregistered",
          registered: false,
          credential: "absent",
        }),
        mdmStatus: async () => ({ state: "unknown" }),
        requestRegistration: vi.fn(async () => ({
          phase: "activationUnknown" as const,
          registered: false,
          credential: "absent" as const,
        })),
        cancelRegistration: vi.fn(async () => {}),
        importRegistration: vi.fn(async () => null),
      },
    },
    selfServicePort: { ...policyFixturePort, execute, confirm: vi.fn() },
    assistantServices: {
      connect: vi.fn(async () => {
        const runtime = {
          initialize: async () => ({ contractVersion: 1, acp: 1 }),
          listSessions: async () => ({ items: [] }),
          connections: async () => ({
            connections: [{ connectionId: "cfg", status: "ready" }],
            preferences: {
              schemaVersion: 1,
              kind: "userPreferences",
              defaultConnectionId: "cfg",
            },
          }),
          savePreferences: async () => ({}),
          createSession: async ({ sessionId }: { sessionId: string }) => ({
            sessionId,
          }),
          restore: async (sessionId: string) => ({
            ...structuredClone(view),
            namespace: { ...view.namespace, sessionId },
          }),
          submit,
          observe: () => vi.fn(),
          close: vi.fn(),
          connection: { closed: new Promise(() => {}) },
        } as unknown as RuntimeClient;
        clients.push(runtime);
        return { runtime, mode: "s1" as const };
      }),
      taskDetails: async () => taskDetail(),
      confirmTask: vi.fn(async () => {}),
      cancelTask: vi.fn(async () => {}),
    },
  };
  input.environment!.account = createLoginFixture(
    async (organization, login) => {
      identity = {
        ...guest(),
        user: {
          schemaVersion: 1,
          kind: "testUser",
          userId: login || "user",
          displayName: login || "user",
          nameKey: login || "user",
        },
        identity: {
          mode: "user_device",
          authorityId: "fixture-enterprise",
          tenantId: organization || "qinghe",
          principalId: login || "user",
          organizationId: organization || "qinghe",
          expiresAtMs: Date.now() + 60000,
        },
      };
      return structuredClone(identity);
    },
  );
  return {
    input,
    submit,
    execute,
    querySubmit,
    list,
    clients,
    current: () => identity,
  };
}
