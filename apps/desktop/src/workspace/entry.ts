import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { accountState } from "../test-users";
import type { SelfServiceCommands } from "../self-service/types";

export type NavigationInput = SelfServiceCommands["navigation"]["input"];
export type NavigationOutput = SelfServiceCommands["navigation"]["output"];
export type NavigationPort = (
  input: NavigationInput,
) => Promise<NavigationOutput>;
export function nativeNavigation(): NavigationPort | undefined {
  const generation = accountState.user?.generation;
  if (!isTauri() || !generation) return undefined;
  return (input) => invoke("navigation", { generation, input });
}
export const listenNavigation = (run: () => void) =>
  listen("navigation-pending", run);
