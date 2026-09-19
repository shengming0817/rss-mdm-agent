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
  const { SurfaceRenderer } = await import("./renderer.js");
  return new SurfaceRenderer(container, options);
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
    rendererFactory: {
      type: Function as PropType<RendererFactory>,
      default: createSurfaceRenderer,
    },
  },
  emits: {
    error: (_error: Error) => true,
    receipt: (_receipt: Receipt) => true,
  },
  setup(props, { emit, expose }) {
    const container = shallowRef<HTMLElement>(),
      error = shallowRef(""),
      errorKind = shallowRef<"renderer" | "action">("renderer");
    let renderer: SurfaceRendererHandle | undefined,
      stop = () => {},
      mounted = false,
      epoch = 0,
      displayed = "";
    const report = (
      failure: Error,
      kind: "renderer" | "action" = "renderer",
    ) => {
      errorKind.value = kind;
      error.value = failure.message;
      emit("error", failure);
    };
    const update = (view?: SessionView) => {
      if (!renderer || !view) return;
      const surface = view.surfaces[props.instanceId];
      if (!surface) {
        renderer.clear();
        displayed = "";
        return;
      }
      const enabled =
        view.connection === "attached" &&
        view.interactions[surface.interactionId]?.status === "pending";
      const key = `${surface.surfaceInstanceId}/${surface.revision}/${enabled}`;
      if (displayed === key) return;
      try {
        renderer.replace(surface, enabled);
        displayed = key;
        if (errorKind.value !== "action") error.value = "";
      } catch {
        renderer.dispose();
        renderer = undefined;
        report(
          new Error(
            "This card cannot be displayed. Use Retry card to reload it.",
          ),
        );
      }
    };
    const mount = async () => {
      if (!mounted || !container.value) return;
      const current = ++epoch;
      stop();
      renderer?.dispose();
      renderer = undefined;
      displayed = "";
      try {
        const created = await props.rendererFactory(container.value, {
          onAction: async (request) => {
            const receipt = await props.runtime.action(request);
            error.value = "";
            emit("receipt", receipt);
          },
          onError: (failure) => report(failure, "action"),
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
        report(
          new Error(
            "Card renderer could not load. Text and standard permissions remain available.",
          ),
        );
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
      renderer?.dispose();
    });
    expose({ retry: () => renderer?.retry(), remount: mount });
    return () =>
      h("div", { class: "rss-ai-surface" }, [
        h("div", { ref: container }),
        error.value ? h("p", { role: "status" }, error.value) : null,
        error.value
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
