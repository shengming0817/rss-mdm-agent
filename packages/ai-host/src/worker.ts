import type {
  Binding,
  ProviderAgentPort,
  ProviderConfiguration,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
/** Implemented by the trusted provider composition module loaded only after activation. */
export type WorkerFactory = (input: {
  configuration: ProviderConfiguration;
  previous: Binding | null;
  tools?: ToolEndpoint;
}) => ProviderAgentPort | Promise<ProviderAgentPort>;
