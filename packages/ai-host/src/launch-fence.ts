import type { Scope } from "./process-contract.js";
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
    scope: Scope,
  ): Promise<Result<void>>;
  releaseLaunch(namespace: Namespace, launchId: Id): Promise<Result<void>>;
  launches(): Promise<Result<readonly WorkerLaunch[]>>;
}

/** Durable launch fence; reservation precedes spawn and SDK activation. */
export type WorkerLaunch = {
  readonly namespace: Namespace;
  readonly launchId: Id;
  readonly artifact: string;
  readonly runtimeDigest: string;
} & (
  | {
      readonly phase: "reserved";
      readonly scope?: never;
    }
  | {
      readonly phase: "registered";
      readonly scope: Scope;
    }
);
export function validLaunch(launch: WorkerLaunch): void {
  namespaceKey(launch.namespace);
  if (
    !isId(launch.launchId) ||
    !/^[a-f0-9]{64}$/.test(launch.runtimeDigest) ||
    typeof launch.artifact !== "string" ||
    !launch.artifact ||
    launch.artifact.length > 4096 ||
    !["reserved", "registered"].includes(launch.phase) ||
    (launch.phase === "registered" && !validScope(launch.scope))
  )
    throw new ContractError("context");
}

export function validScope(scope: Scope): boolean {
  return (
    !!scope &&
    (scope.kind === "processGroup"
      ? Object.keys(scope).length === 2 &&
        Number.isSafeInteger(scope.root) &&
        scope.root > 1 &&
        scope.root <= 2147483647
      : scope.kind === "jobObject" &&
        Object.keys(scope).length === 3 &&
        Number.isSafeInteger(scope.session) &&
        scope.session >= 0 &&
        scope.session <= 4294967295 &&
        /^Local\\rss-mdm-worker-[a-f0-9-]{36}$/.test(scope.name))
  );
}
