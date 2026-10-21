import type { SelfServiceMetadata } from "./types";

// Presentation is open producer data. Only strings become UI copy, never execution authority.
export function presentationText(
  metadata: SelfServiceMetadata | undefined,
  key: string,
): string {
  const value = metadata?.presentation[key];
  return typeof value === "string"
    ? value
    : key === "displayName"
      ? "策略"
      : "";
}
