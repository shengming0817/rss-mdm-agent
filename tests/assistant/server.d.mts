import type { Plugin } from "vite";
export function createFixture(): Promise<{
  plugin: Plugin;
  seed(count?: number): Promise<void>;
  close(): Promise<void>;
}>;
