import { RendererError } from "./errors.js";
export { questions } from "./questions.js";
export type { Question } from "./questions.js";
export { RendererError } from "./errors.js";
export type { RendererErrorCode } from "./errors.js";
import {
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  onScopeDispose,
  shallowRef,
  watch,
  type PropType,
} from "vue";
import type { RuntimeClient, SessionView } from "@rss-mdm-agent/ai-client";
import type { Receipt } from "@rss-mdm-agent/ai-contract";
import type { SurfaceRendererHandle, RendererOptions } from "./renderer.js";
export type { RendererOptions, SurfaceRendererHandle } from "./renderer.js";
export type RendererFactory = (
  container: HTMLElement,
  options: RendererOptions,
) => Promise<SurfaceRendererHandle>;
/** Lazy import makes renderer load failure a display error; text and ACP remain usable. */
export const createSurfaceRenderer: RendererFactory = async (
  container,
  options,
) => {
  try {
    const { SurfaceRenderer } = await import("./renderer.js");
    return new SurfaceRenderer(container, options);
  } catch {
    throw new RendererError("renderer_load");
  }
};
export function useSessionView(runtime: RuntimeClient, sessionId: string) {
  const view = shallowRef<SessionView | undefined>(
    runtime.getSession(sessionId),
  );
  const dispose = runtime.observe((next) => {
    if (next.namespace.sessionId === sessionId) view.value = next;
  });
  onScopeDispose(dispose);
  return view;
}
/** Ready-to-mount Vue seam: projection in, official component, bounded action out. */
export const RuntimeSurface = defineComponent({
  name: "RuntimeSurface",
  props: {
    runtime: { type: Object as PropType<RuntimeClient>, required: true },
    sessionId: { type: String, required: true },
    instanceId: { type: String, required: true },
    now: { type: Function as PropType<() => number>, default: Date.now },
    rendererFactory: {
      type: Function as PropType<RendererFactory>,
      default: createSurfaceRenderer,
    },
  },
  emits: {
    error: (_error: RendererError) => true,
    receipt: (_receipt: Receipt) => true,
  },
  setup(props, { emit, expose, slots }) {
    const container = shallowRef<HTMLElement>(),
      error = shallowRef(""),
      stateNote = shallowRef(""),
      errorKind = shallowRef<"renderer" | "action">("renderer");
    let renderer: SurfaceRendererHandle | undefined,
      stop = () => {},
      mounted = false,
      epoch = 0,
      displayed = "";
    let deadline: ReturnType<typeof setTimeout> | undefined;
    const stopDeadline = () => {
      clearTimeout(deadline);
      deadline = undefined;
    };
    const report = (
      failure: RendererError,
      kind: "renderer" | "action" = "renderer",
    ) => {
      errorKind.value = kind;
      error.value = failure.message;
      emit("error", failure);
    };
    const update = (view?: SessionView) => {
      stopDeadline();
      if (!renderer || !view) return;
      const surface = view.surfaces[props.instanceId];
      if (!surface) {
        renderer.clear();
        error.value = "";
        stateNote.value = "";
        displayed = "";
        return;
      }
      const interaction = view.interactions[surface.interactionId];
      const connected =
        view.connection === "attached" &&
        view.generation === surface.generation;
      const expired = !!interaction && props.now() > interaction.expiresAtMs;
      stateNote.value =
        interaction?.status === "answered"
          ? "This question has already been answered."
          : interaction?.status === "expired" ||
              (interaction?.status === "pending" && expired)
            ? "This question has expired."
            : interaction?.status === "unavailable"
              ? "This question is no longer available."
              : view.connection !== "attached"
                ? "Reconnect the session to respond."
                : !connected || !interaction
                  ? "This question is no longer available."
                  : "";
      if (interaction?.status === "pending" && !expired)
        deadline = setTimeout(
          () => update(props.runtime.getSession(props.sessionId)),
          Math.min(
            2_147_483_647,
            Math.max(1, interaction.expiresAtMs - props.now() + 1),
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
        if (errorKind.value !== "action" || !renderer.canRetry)
          error.value = "";
      } catch {
        renderer.dispose();
        renderer = undefined;
        report(new RendererError("renderer_invalid"));
      }
    };
    const mount = async () => {
      if (!mounted || !container.value) return;
      const current = ++epoch;
      stop();
      stopDeadline();
      renderer?.dispose();
      renderer = undefined;
      displayed = "";
      try {
        const created = await props.rendererFactory(container.value, {
          now: props.now,
          onAction: async (request) => {
            const receipt = await props.runtime.action(request);
            if (current === epoch && mounted) {
              error.value = "";
              emit("receipt", receipt);
            }
          },
          onError: (failure) => {
            if (current === epoch && mounted) report(failure, "action");
          },
        });
        if (current !== epoch || !mounted) {
          created.dispose();
          return;
        }
        renderer = created;
        stop = props.runtime.observe((view) => {
          if (view.namespace.sessionId === props.sessionId) update(view);
        });
        update(props.runtime.getSession(props.sessionId));
      } catch {
        report(new RendererError("renderer_load"));
      }
    };
    onMounted(() => {
      mounted = true;
      void mount();
    });
    watch(
      () => [props.runtime, props.sessionId, props.instanceId],
      () => void mount(),
    );
    onBeforeUnmount(() => {
      mounted = false;
      epoch++;
      stop();
      stopDeadline();
      renderer?.dispose();
    });
    expose({ retry: () => renderer?.retry(), remount: mount });
    return () =>
      h("div", { class: "rss-ai-surface" }, [
        h("div", { ref: container }),
        stateNote.value ? h("p", { role: "status" }, stateNote.value) : null,
        error.value
          ? h(
              "p",
              { role: "status", class: "rss-ai-surface-error" },
              error.value,
            )
          : null,
        error.value && errorKind.value === "renderer"
          ? slots.fallback?.()
          : null,
        error.value && (errorKind.value === "renderer" || renderer?.canRetry)
          ? h(
              "button",
              {
                type: "button",
                onClick: () =>
                  errorKind.value === "action" ? renderer?.retry() : mount(),
              },
              errorKind.value === "action" ? "Retry response" : "Retry card",
            )
          : null,
      ]);
  },
});
