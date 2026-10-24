import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
/** Only the native owner issues these coordinates. This key is isolation, not authorization. */
export const workspaceIdentity = (value?: UserContext) =>
  JSON.stringify(
    value
      ? [value.generation, value.identity, value.executionBinding ?? null]
      : ["anonymous"],
  );
