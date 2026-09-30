import { reactive } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppearanceSnapshot } from "./self-service/types";
const solid = (): AppearanceSnapshot => ({
  materialEnabled: false,
  reducedMotion: false,
  highContrast: false,
});
export function createAppearance(
  read: (() => Promise<AppearanceSnapshot>) | undefined = isTauri()
    ? () => invoke<AppearanceSnapshot>("appearance_snapshot")
    : undefined,
) {
  const state = reactive(solid());
  let disposed = false,
    reading = false;
  async function refresh() {
    if (!read || disposed || reading) return;
    reading = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const next = await Promise.race([
        read(),
        new Promise<never>((_, reject) => {
          timer = setTimeout(
            () => reject(new Error("appearance_timeout")),
            3000,
          );
        }),
      ]);
      if (
        ![next.materialEnabled, next.reducedMotion, next.highContrast].every(
          (value) => typeof value === "boolean",
        )
      )
        throw new Error("invalid_appearance");
      if (!disposed) Object.assign(state, next);
    } catch {
      if (!disposed) Object.assign(state, { ...solid(), reducedMotion: true });
    } finally {
      if (timer) clearTimeout(timer);
      reading = false;
    }
  }
  return {
    state,
    refresh,
    dispose() {
      disposed = true;
      Object.assign(state, solid());
    },
  };
}
