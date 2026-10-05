import type { Binding } from "@rss-mdm-agent/ai-contract/wire";
import type {
  ProviderAgentPort,
  ProviderConfiguration,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract/ports";
/** Implemented by the trusted provider composition module loaded only after activation. */
export type WorkerFactory = (input: {
  configuration: ProviderConfiguration;
  previous: Binding | null;
  /** Transient provider composition data; never stored in launch fences or bindings. */
  activation?: unknown;
  tools?: ToolEndpoint;
}) => ProviderAgentPort | Promise<ProviderAgentPort>;
