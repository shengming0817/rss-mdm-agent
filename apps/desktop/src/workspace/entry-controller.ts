import { proxy } from "valtio/vanilla";
import type { NavigationOutput, NavigationPort } from "./entry";
/** Reads and resolves the native queue; only the destination page decides what it can display. */
export function createEntryNavigation(
  port: NavigationPort | undefined,
  listen: ((notify: () => void) => Promise<() => void>) | undefined,
  open: (result: NavigationOutput) => Promise<boolean>,
) {
  const state = proxy({ notice: "", pending: "" });
  let active = true,
    started = false,
    reading = false,
    dirty = false,
    stop = () => {};
  async function drain() {
    if (!active || !port) return;
    if (reading) {
      dirty = true;
      return;
    }
    reading = true;
    try {
      do {
        dirty = false;
        const next = await port({ kind: "next" });
        if (!active) return;
        if (next.kind !== "pending" || !next.value) return;
        state.pending = next.value.id;
        const resolved = await port({
          kind: "resolve",
          intent: next.value.intent,
        });
        if (!active) return;
        if (!(await open(resolved))) {
          if (active)
            state.notice =
              "当前入口暂不可用；未执行任何操作。可以稍后重试或忽略此入口。";
          return;
        }
        if (!active) return;
        await port({ kind: "ack", id: next.value.id });
        if (!active) return;
        state.pending = "";
        state.notice = "";
        dirty = true;
      } while (active && dirty);
    } catch {
      if (active)
        state.notice =
          "入口无法核验，请检查当前账号与目标权限；未执行任何操作。";
    } finally {
      reading = false;
      if (active && dirty && !state.notice) void drain();
    }
  }
  async function start() {
    if (!active || !port || started) return;
    started = true;
    try {
      const release = await listen?.(() => void drain());
      if (!active) {
        release?.();
        return;
      }
      stop = release ?? (() => {});
      await drain();
    } catch {
      if (active) state.notice = "无法监听本机入口，请重新打开应用后重试。";
    }
  }
  return {
    state,
    start,
    retry: drain,
    async dismiss() {
      if (!active || !port || !state.pending) return;
      try {
        await port({ kind: "ack", id: state.pending });
        if (active) {
          state.pending = "";
          state.notice = "";
          await drain();
        }
      } catch {
        if (active) state.notice = "忽略入口未完成，请重试。";
      }
    },
    dispose() {
      active = false;
      dirty = false;
      stop();
      state.pending = "";
      state.notice = "";
    },
  };
}
