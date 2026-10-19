// ref: Tauri tauri-v2.11.2 crates/tauri/src/webview/webview_window.rs;
// @tauri-apps/api 2.11.1 window.ts — current-window controls only.
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function nativeWindow() {
  if (!isTauri()) return undefined;
  return {
    minimize: () => getCurrentWindow().minimize(),
    maximize: () => getCurrentWindow().toggleMaximize(),
    close: () => getCurrentWindow().close(),
  };
}
