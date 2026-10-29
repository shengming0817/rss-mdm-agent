import { useLayoutEffect, useRef, useState } from "react";
/** Reading position follows output only while the reader remains at the bottom. */
export function useConversationScroll(
  scope: string,
  version: string,
  active: boolean,
) {
  const viewport = useRef<HTMLDivElement>(null),
    content = useRef<HTMLDivElement>(null),
    positions = useRef(new Map<string, { top: number; pinned: boolean }>()),
    pinned = useRef(true),
    observed = useRef({ scope: "", version: "" }),
    [newOutput, setNewOutput] = useState(false);
  useLayoutEffect(() => {
    const node = viewport.current,
      body = content.current;
    if (!node || !body || !active) return;
    let alive = true;
    const saved = positions.current.get(scope);
    pinned.current = saved?.pinned ?? true;
    node.scrollTop = pinned.current ? node.scrollHeight : (saved?.top ?? 0);
    setNewOutput(false);
    const read = () => {
      pinned.current =
        node.scrollHeight - node.clientHeight - node.scrollTop <= 72;
      positions.current.set(scope, {
        top: node.scrollTop,
        pinned: pinned.current,
      });
      if (pinned.current) setNewOutput(false);
    };
    let height = body.offsetHeight;
    const observer = new ResizeObserver(() => {
      if (!alive || viewport.current !== node) return;
      const changed = body.offsetHeight !== height;
      height = body.offsetHeight;
      if (pinned.current) node.scrollTop = node.scrollHeight;
      else if (changed) setNewOutput(true);
    });
    node.addEventListener("scroll", read, { passive: true });
    observer.observe(body);
    return () => {
      alive = false;
      positions.current.set(scope, {
        top: node.scrollTop,
        pinned: pinned.current,
      });
      node.removeEventListener("scroll", read);
      observer.disconnect();
    };
  }, [scope, active]);
  useLayoutEffect(() => {
    const node = viewport.current;
    if (!active || !node) return;
    const previous = observed.current;
    observed.current = { scope, version };
    if (pinned.current) node.scrollTop = node.scrollHeight;
    else if (previous.scope === scope && previous.version !== version)
      setNewOutput(true);
  }, [scope, version, active]);
  return {
    viewport,
    content,
    newOutput,
    latest() {
      const node = viewport.current;
      if (!node) return;
      pinned.current = true;
      node.scrollTop = node.scrollHeight;
      positions.current.set(scope, { top: node.scrollTop, pinned: true });
      setNewOutput(false);
    },
  };
}
