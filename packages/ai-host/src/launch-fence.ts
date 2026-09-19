import {
  ContractError,
  isId,
  type Id,
  type Namespace,
  type Result,
} from "@rss-mdm-agent/ai-contract";
import { namespaceKey } from "@rss-mdm-agent/ai-contract/transitions";

/** Host process-lifecycle persistence; independent of portable session storage. */
export interface WorkerLaunchFenceStore {
  reserveLaunch(launch: WorkerLaunch): Promise<Result<void>>;
  registerLaunch(
    namespace: Namespace,
    launchId: Id,
    rootPid: number,
    pgid: number,
  ): Promise<Result<void>>;
  releaseLaunch(namespace: Namespace, launchId: Id): Promise<Result<void>>;
  launches(): Promise<Result<readonly WorkerLaunch[]>>;
}

/** Durable launch fence; reservation precedes spawn and SDK activation. */
export type WorkerLaunch = {
  readonly namespace: Namespace;
  readonly launchId: Id;
  readonly artifact: string;
} & (
  | {
      readonly phase: "reserved";
      readonly rootPid?: never;
      readonly pgid?: never;
    }
  | {
      readonly phase: "registered";
      readonly rootPid: number;
      readonly pgid: number;
    }
);
export function validLaunch(launch: WorkerLaunch): void {
  namespaceKey(launch.namespace);
  if (
    !isId(launch.launchId) ||
    typeof launch.artifact !== "string" ||
    !launch.artifact ||
    launch.artifact.length > 4096 ||
    !["reserved", "registered"].includes(launch.phase) ||
    (launch.phase === "registered" &&
      (!Number.isSafeInteger(launch.rootPid) ||
        launch.rootPid <= 1 ||
        launch.pgid !== launch.rootPid))
  )
    throw new ContractError("context");
}
