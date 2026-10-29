import {
  createElement as h,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { RuntimeClient, SessionView } from "@rss-mdm-agent/ai-client";
import type { Receipt } from "@rss-mdm-agent/ai-contract/wire";
import type { NodeMount } from "./operations/surface.js";
import type { SurfaceRendererHandle } from "./renderer.js";
import { RendererError } from "./errors.js";
import { createSurfaceRenderer, type RendererFactory } from "./index.js";

export function useSessionView(runtime: RuntimeClient, sessionId: string) {
  const [projection, setProjection] = useState(() => ({
    runtime,
    sessionId,
    view: runtime.getSession(sessionId),
  }));
  useEffect(() => {
    const update = (view: SessionView | undefined) =>
      setProjection({ runtime, sessionId, view });
    const stop = runtime.observe((view) => {
      if (view.namespace.sessionId === sessionId) update(view);
    });
    update(runtime.getSession(sessionId));
    return stop;
  }, [runtime, sessionId]);
  // Changing the owner must never show the previous owner's projection for one render.
  return projection.runtime === runtime && projection.sessionId === sessionId
    ? projection.view
    : runtime.getSession(sessionId);
}
export interface RuntimeSurfaceProps {
  runtime: RuntimeClient;
  sessionId: string;
  instanceId: string;
  mount: NodeMount;
  now?: () => number;
  rendererFactory?: RendererFactory;
  onError?(error: RendererError): void;
  onReceipt?(receipt: Receipt): void;
  fallback?: ReactNode;
}
/** Projection in, official bounded renderer, authenticated action out. No execution authority. */
export function RuntimeSurface({
  runtime,
  sessionId,
  instanceId,
  mount,
  now = Date.now,
  rendererFactory = createSurfaceRenderer,
  onError,
  onReceipt,
  fallback,
}: RuntimeSurfaceProps) {
  const container = useRef<HTMLDivElement>(null);
  const current = useRef<SurfaceRendererHandle | undefined>(undefined);
  const callbacks = useRef({ onError, onReceipt });
  callbacks.current = { onError, onReceipt };
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState({
    owner: runtime,
    sessionId,
    instanceId,
    note: "",
    error: "",
    kind: "renderer" as "renderer" | "action",
  });
  useEffect(() => {
    let active = true,
      renderer: SurfaceRendererHandle | undefined,
      stop = () => {},
      displayed = "";
    let deadline: ReturnType<typeof setTimeout> | undefined;
    let errorKind: "renderer" | "action" = "renderer";
    const patch = (values: Partial<typeof status>) => {
      if (active)
        setStatus((old) => ({
          ...old,
          owner: runtime,
          sessionId,
          instanceId,
          ...values,
        }));
    };
    patch({ note: "", error: "" });
    const report = (
      error: RendererError,
      kind: "renderer" | "action" = "renderer",
    ) => {
      if (!active) return;
      errorKind = kind;
      patch({ error: error.message, kind });
      callbacks.current.onError?.(error);
    };
    const update = (view: SessionView | undefined) => {
      clearTimeout(deadline);
      if (!renderer || !active) return;
      const surface = view?.surfaces[instanceId];
      if (!surface || !view) {
        renderer.clear();
        displayed = "";
        patch({ note: "", error: "" });
        return;
      }
      const interaction = view.interactions[surface.interactionId];
      const connected =
        view.connection === "attached" &&
        view.generation === surface.generation;
      const expired = !!interaction && now() > interaction.expiresAtMs;
      patch({
        note:
          interaction?.status === "answered"
            ? "此问题已经回答。"
            : interaction?.status === "expired" ||
                (interaction?.status === "pending" && expired)
              ? "此问题已过期。"
              : interaction?.status === "unavailable"
                ? "此问题已不可用。"
                : view.connection !== "attached"
                  ? "请重新连接会话后回答。"
                  : !connected || !interaction
                    ? "此问题已不可用。"
                    : "",
      });
      if (interaction?.status === "pending" && !expired)
        deadline = setTimeout(
          () => update(runtime.getSession(sessionId)),
          Math.min(
            2147483647,
            Math.max(1, interaction.expiresAtMs - now() + 1),
          ),
        );
      const key = JSON.stringify([
        surface.surfaceInstanceId,
        surface.revision,
        connected,
        interaction,
        expired,
      ]);
      if (displayed === key) return;
      try {
        renderer.replace(surface, interaction, connected);
        displayed = key;
        if (errorKind !== "action" || !renderer.canRetry) patch({ error: "" });
      } catch {
        renderer.dispose();
        renderer = undefined;
        current.current = undefined;
        report(new RendererError("renderer_invalid"));
      }
    };
    void rendererFactory(container.current!, {
      now,
      mount,
      onAction: async (request) => {
        const receipt = await runtime.action(request);
        if (active) {
          patch({ error: "" });
          callbacks.current.onReceipt?.(receipt);
        }
      },
      onError: (error) => report(error, "action"),
    })
      .then((created) => {
        if (!active) {
          created.dispose();
          return;
        }
        renderer = created;
        current.current = created;
        stop = runtime.observe((view) => {
          if (view.namespace.sessionId === sessionId) update(view);
        });
        update(runtime.getSession(sessionId));
      })
      .catch(() => report(new RendererError("renderer_load")));
    return () => {
      active = false;
      stop();
      clearTimeout(deadline);
      renderer?.dispose();
      current.current = undefined;
    };
  }, [runtime, sessionId, instanceId, mount, now, rendererFactory, attempt]);
  const visible =
    status.owner === runtime &&
    status.sessionId === sessionId &&
    status.instanceId === instanceId;
  return h(
    "div",
    { className: "rss-ai-surface" },
    h("div", { ref: container }),
    visible && status.note ? h("p", { role: "status" }, status.note) : null,
    visible && status.error ? h("p", { role: "status" }, status.error) : null,
    visible && status.error && status.kind === "renderer" ? fallback : null,
    visible &&
      status.error &&
      (status.kind === "renderer" || current.current?.canRetry)
      ? h(
          "button",
          {
            type: "button",
            onClick: () => {
              if (status.kind === "action") void current.current?.retry();
              else setAttempt((value) => value + 1);
            },
          },
          status.kind === "action" ? "重试原回答" : "重新加载卡片",
        )
      : null,
  );
}
