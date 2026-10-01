// The wrapper selects assembly before startup; errors never change it.
export const fixtureSelected =
  import.meta.env.DEV && import.meta.env.VITE_RSS_ASSEMBLY === "fixture";

import { invoke, isTauri } from "@tauri-apps/api/core";
export async function verifyAssembly() {
  if (import.meta.env.DEV && isTauri()) {
    const mode = await invoke("development_assembly");
    if (mode !== (fixtureSelected ? "fixture" : "production"))
      throw new Error("desktop assembly mismatch");
  }
}
