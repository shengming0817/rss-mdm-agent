import "@a2ui/lit/v0_9";
import {
  Catalog,
  MessageProcessor,
  type A2uiMessage,
} from "@a2ui/web_core/v0_9";
import {
  A2uiText,
  A2uiColumn,
  A2uiButton,
  A2uiTextField,
} from "@a2ui/web_core/v0_9/basic_catalog";
import {
  interactionCatalog,
  validateSurface,
  type ActionRequest,
  type SurfaceState,
} from "@rss-mdm-agent/ai-contract";

const catalog = new Catalog(
  interactionCatalog.catalogId,
  [A2uiText, A2uiColumn, A2uiButton, A2uiTextField],
  [],
);
export interface RendererOptions {
  onAction(request: ActionRequest): Promise<void>;
  onError(error: Error): void;
  now?: () => number;
}
/** Disposable official renderer cache. All recovery truth remains in ai-client. */
export class SurfaceRenderer {
  private processor?: MessageProcessor<typeof A2uiText>;
  private state?: SurfaceState;
  private pending?: ActionRequest;
  private sending = false;
  private disposed = false;
  constructor(
    private container: HTMLElement,
    private options: RendererOptions,
  ) {}
  replace(input: SurfaceState, enabled = true): void {
    if (this.disposed) throw new Error("renderer disposed");
    const state = validateSurface(input);
    this.clear();
    this.state = state;
    const processor = new MessageProcessor(
      [catalog],
      async (message) => {
        if (
          this.disposed ||
          this.processor !== processor ||
          !enabled ||
          this.sending
        )
          return;
        const now = (this.options.now ?? Date.now)();
        this.pending = {
          schemaVersion: 2,
          kind: "actionRequest",
          metadata: {
            schemaVersion: 2,
            kind: "surfaceAction",
            sessionId: state.namespace.sessionId,
            commandId: crypto.randomUUID(),
            interactionId: state.interactionId,
            surfaceInstanceId: state.surfaceInstanceId,
            surfaceRevision: state.revision,
            generation: state.generation,
            nativeRunId: state.nativeRunId,
          },
          message: { version: interactionCatalog.version, action: message },
          expiresAtMs: now + 30_000,
        };
        await this.retry();
      },
      { version: interactionCatalog.version },
    );
    this.processor = processor;
    try {
      processor.processMessages(state.messages as unknown as A2uiMessage[]);
      const model = processor.model.getSurface(state.surfaceId);
      if (state.status === "deleted") {
        if (model) throw new Error("renderer deletion failed");
        return;
      }
      if (!model || !model.componentsModel.get("root"))
        throw new Error("renderer surface unavailable");
      const element = document.createElement("a2ui-surface") as HTMLElement & {
        surface: typeof model;
      };
      element.surface = model;
      element.inert = !enabled;
      this.container.replaceChildren(element);
    } catch {
      this.clear();
      throw new Error(
        "A2UI renderer unavailable; restore the session to retry",
      );
    }
  }
  /** Retry uses exactly the same command ID, expiry and action payload. */
  async retry(): Promise<void> {
    if (!this.pending || this.sending || this.disposed) return;
    this.sending = true;
    try {
      await this.options.onAction(structuredClone(this.pending));
      this.pending = undefined;
    } catch {
      this.options.onError(
        new Error("Response was not accepted; retry or restore the session"),
      );
    } finally {
      this.sending = false;
    }
  }
  clear(): void {
    this.processor?.model.dispose();
    this.processor = undefined;
    this.container.replaceChildren();
    this.pending = undefined;
  }
  dispose(): void {
    this.disposed = true;
    this.clear();
    this.state = undefined;
  }
}
