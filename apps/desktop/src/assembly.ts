// The wrapper selects assembly before startup; errors never change it.
export const fixtureSelected =
  import.meta.env.DEV && import.meta.env.VITE_RSS_ASSEMBLY === "fixture";

import { invoke, isTauri } from "@tauri-apps/api/core";
export async function verifyAssembly() {
  if (import.meta.env.DEV && isTauri()) {
    const mode = await invoke("development_assembly");
    if (mode !== (fixtureSelected ? "fixture" : "production"))
      throw new Error("desktop assembly mismatch");
  }
}

import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
import type { ServicePort, nativeHost } from "./settings/native";
import type { AssistantServices } from "./assistant/controller";
import type {
  DataAccessPort,
  SoftwarePort,
  SelfServicePort,
} from "./self-service/types";
export type LocalPorts = {
  tasks?: import("./self-service/types").TaskPort | null;
  data: DataAccessPort | null;
  software: SoftwarePort | null;
  settings: import("./system-settings/controller").SettingsPort | null;
  network: import("./self-service/native").NetworkCheckPort | null;
};
export type FixtureEnvironment = {
  local: LocalPorts;
  device: import("./home/device").DeviceOverviewPort;
  kind: "fixture";
  account?: import("./account/types").FixtureLoginPort;
  identity: { read(): Promise<UserContext>; guest(): Promise<UserContext> };
  host: NonNullable<ReturnType<typeof nativeHost>>;
  service: ServicePort;
};
export function assertFixtureAssembly(input: {
  environment?: FixtureEnvironment;
  assistantServices?: AssistantServices;
  selfServicePort?: SelfServicePort;
}) {
  if (!input.environment) return;
  if (
    !input.assistantServices?.connect ||
    !input.assistantServices.taskDetails ||
    !input.assistantServices.confirmTask ||
    !input.assistantServices.cancelTask ||
    !input.environment.local.tasks?.list ||
    !input.selfServicePort?.execute ||
    !input.environment.local.tasks.control ||
    !input.environment.local ||
    !input.environment.identity?.read ||
    !input.environment.device?.read ||
    !input.environment.host?.read ||
    !input.environment.host.restart ||
    !input.environment.service?.read ||
    !input.environment.service.registration ||
    !input.environment.service.mdmStatus ||
    !input.environment.service.requestRegistration
  )
    throw new Error("incomplete fixture assembly");
}
