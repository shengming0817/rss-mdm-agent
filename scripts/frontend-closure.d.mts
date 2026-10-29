import type { Plugin } from "vite";
export function frontendClosure(owner: "desktop" | "ui"): Plugin;
export function verifyFrontendClosure(
  directory: string,
  expectedSource?: Record<string, string>,
): Record<string, unknown>;
export function assertFrontendClosure(
  modules: string[],
  files: Record<string, Uint8Array>,
): void;
