import { useMemo, useState } from "react";
import {
  CloseOutlined,
  ExpandOutlined,
  MinusOutlined,
} from "@ant-design/icons";
import { nativeWindow } from "../workspace/window";
export function WindowControls() {
  const controls = useMemo(nativeWindow, []),
    [error, setError] = useState("");
  async function act(action: () => Promise<void>) {
    try {
      await action();
      setError("");
    } catch {
      setError("窗口操作未完成，请重试。");
    }
  }
  return (
    <div className="rss-window-controls" title={error || undefined}>
      <button
        type="button"
        aria-label="最小化窗口"
        disabled={!controls}
        onClick={() => controls && void act(controls.minimize)}
      >
        <MinusOutlined />
      </button>
      <button
        type="button"
        aria-label="最大化或还原窗口"
        disabled={!controls}
        onClick={() => controls && void act(controls.maximize)}
      >
        <ExpandOutlined />
      </button>
      <button
        type="button"
        aria-label="关闭窗口"
        disabled={!controls}
        onClick={() => controls && void act(controls.close)}
      >
        <CloseOutlined />
      </button>
      {error && (
        <span role="status" className="rss-visually-hidden">
          {error}
        </span>
      )}
    </div>
  );
}
