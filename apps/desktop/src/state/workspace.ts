import { subscribe } from "valtio/vanilla";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import { assertFixtureAssembly, type FixtureEnvironment } from "../assembly";
import {
  createAssistant,
  type AssistantServices,
} from "../assistant/controller";
import { nativeAssistant } from "../assistant/native";
import { createTaskCenter } from "../task-center/controller";
import { createSoftware } from "../software/controller";
import { createSystemSettings } from "../system-settings/controller";
import { createDirectory } from "../self-service/directory";
import { createPolicyPreparation } from "../self-service/policy-preparation";
import { createNetworkCheck } from "../device-tools/controller";
import {
  nativeDataPort,
  nativeNetworkCheckPort,
  nativePort,
  nativeSettingsPort,
  nativeSoftwarePort,
  nativeTaskPort,
  nativeDeviceAccessPort,
} from "../self-service/native";
import type { SelfServicePort } from "../self-service/types";
import { createDeviceOverview } from "../home/device";
import { nativeDeviceOverview } from "../home/native";
import { createHostSettings } from "../settings/controller";
import { createDiagnostics } from "../settings/diagnostics";
import { createConnectionSettings } from "../settings/connections";
import { createPersonalVault } from "../settings/personal-vault";
import {
  nativeDiagnostics,
  nativeHost,
  nativePersonalVault,
  nativeConnectionSave,
  nativeAppVersion,
} from "../settings/native";
import { workspaceIdentity } from "../workspace/identity";
import { createNavigation } from "./navigation";

export interface WorkspaceInput {
  environment?: FixtureEnvironment;
  assistantServices?: AssistantServices;
  selfServicePort?: SelfServicePort;
}
/** One subject-bound composition. Ports retain their original backend execution authority. */
export function createWorkspace(
  identity: UserContext,
  input: WorkspaceInput = {},
) {
  assertFixtureAssembly(input);
  if (input.environment && !import.meta.env.DEV)
    throw Error("fixture is development-only");
  const key = workspaceIdentity(identity),
    environment = input.environment;
  const local = environment
    ? environment.local
    : {
        tasks: nativeTaskPort(),
        data: nativeDataPort(),
        software: nativeSoftwarePort(),
        settings: nativeSettingsPort(),
        network: nativeNetworkCheckPort(),
      };
  const selfService = environment ? input.selfServicePort! : nativePort();
  const tasks = createTaskCenter(local.tasks ?? null, selfService);
  const software = createSoftware(local.data, local.software, tasks);
  const system = createSystemSettings(
    local.settings,
    local.data,
    undefined,
    tasks,
  );
  const directory = createDirectory(tasks.port);
  const policies = createPolicyPreparation(selfService);
  const networkCheck = createNetworkCheck(local.network);
  const assistant = createAssistant(
    environment ? input.assistantServices : nativeAssistant(tasks),
    () => crypto.randomUUID(),
    Date.now,
    key,
  );
  const device = createDeviceOverview(
    environment ? environment.device : nativeDeviceOverview(),
  );
  const host = createHostSettings(
    environment ? environment.host : nativeHost(),
  );
  const diagnostics = createDiagnostics(
    environment ? environment.diagnostics : nativeDiagnostics(),
  );
  const navigation = createNavigation();
  const deviceAccess = environment ? undefined : nativeDeviceAccessPort();
  const personalVault = environment
    ? environment.personalVault
    : nativePersonalVault();
  const vault = createPersonalVault(personalVault);
  const connections = createConnectionSettings(
    assistant,
    environment ? environment.connectionSave : nativeConnectionSave(),
  );
  let alive = true,
    started = false;
  const visibility = () =>
    tasks.setVisible(
      navigation.state.page === "home" || navigation.state.page === "tasks",
      navigation.state.page === "home",
    );
  const stopNavigation = subscribe(
    navigation.state,
    () => {
      if (alive) visibility();
    },
    true,
  );
  return {
    key,
    identity,
    navigation,
    tasks,
    software,
    system,
    directory,
    policies,
    networkCheck,
    assistant,
    device,
    host,
    diagnostics,
    network: local.network,
    deviceAccess,
    personalVault,
    vault,
    connections,
    appVersion: environment ? environment.appVersion : nativeAppVersion(),
    start() {
      if (!alive || started) return;
      started = true;
      visibility();
      void assistant.connect();
      void device.refresh();
      void host.refresh();
    },
    dispose() {
      if (!alive) return;
      alive = false;
      stopNavigation();
      assistant.dispose();
      directory.dispose();
      policies.dispose();
      networkCheck.dispose();
      software.dispose();
      system.dispose();
      tasks.dispose();
      device.dispose();
      host.dispose();
      diagnostics.dispose();
      vault.dispose();
      connections.dispose();
    },
  };
}
export type Workspace = ReturnType<typeof createWorkspace>;
