import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { accountState } from "../test-users";
import type { SelfServiceCommands } from "../self-service/types";

export type NavigationInput = SelfServiceCommands["navigation"]["input"];
export type NavigationOutput = SelfServiceCommands["navigation"]["output"];
export type NavigationPort = (
  input: NavigationInput,
) => Promise<NavigationOutput>;
export const nativeNavigation = (): NavigationPort | undefined =>
  isTauri()
    ? (input) =>
        invoke("navigation", {
          generation: accountState.user?.generation,
          input,
        })
    : undefined;
export const listenNavigation = (run: () => void) =>
  listen("navigation-pending", run);
