import type { Binding, Namespace } from "@rss-mdm-agent/ai-contract/wire";
import type {
  Budget,
  ProviderAgentPort,
  ProviderConfiguration,
  Result,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract/ports";
import type { WorkerLaunchFenceStore } from "./launch-fence.js";
import type { Scope } from "./process-contract.js";

/** Only the operations required by Host admission and lifecycle ownership. */
export interface WorkerHandle extends ProviderAgentPort {
  readonly launchId: string;
  onFailure?: () => void;
  start(
    configuration: ProviderConfiguration,
    budget: Budget,
    previous?: Binding | null,
  ): Promise<Result<void>>;
  admitTools(): void;
  terminate(): void;
}
export interface WorkerAdapter {
  create(
    store: WorkerLaunchFenceStore,
    namespace: Namespace,
    artifact: string,
    bridge?: ToolEndpoint,
    activation?: unknown,
  ): WorkerHandle;
  absent(scope: Scope, budget: Budget): Promise<boolean>;
}
