import type {
  ProviderAgentPort,
  ProviderConfiguration,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
/** Implemented by the trusted provider composition module loaded only after activation. */
export type WorkerFactory = (input: {
  configuration: ProviderConfiguration;
  tools?: ToolEndpoint;
}) => ProviderAgentPort | Promise<ProviderAgentPort>;
