import { ClientError, type InteractionView } from "@rss-mdm-agent/ai-client";
import { RendererError } from "./errors.js";
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
  onError(error: RendererError): void;
  now?: () => number;
}
export interface SurfaceRendererHandle {
  replace(
    input: SurfaceState,
    interaction: InteractionView | undefined,
    connected: boolean,
  ): void;
  readonly canRetry: boolean;
  retry(): Promise<void>;
  clear(): void;
  dispose(): void;
}
/** Disposable official renderer cache. All recovery truth remains in ai-client. */
export class SurfaceRenderer implements SurfaceRendererHandle {
  private processor?: MessageProcessor<typeof A2uiText>;
  private pending?: ActionRequest;
  private sending = false;
  private disposed = false;
  private interaction?: InteractionView;
  private connected = false;
  private now(): number {
    return (this.options.now ?? Date.now)();
  }
  private pendingInteraction(): boolean {
    return (
      this.interaction?.status === "pending" &&
      this.now() <= this.interaction.expiresAtMs
    );
  }
  get canRetry(): boolean {
    return (
      !!this.pending &&
      !this.disposed &&
      this.connected &&
      (this.pendingInteraction() ||
        (this.interaction?.status === "answered" &&
          this.interaction.responseCommandId ===
            this.pending.metadata.commandId))
    );
  }
  constructor(
    private container: HTMLElement,
    private options: RendererOptions,
  ) {}
  replace(
    input: SurfaceState,
    interaction: InteractionView | undefined,
    connected: boolean,
  ): void {
    if (this.disposed) throw new RendererError("renderer_disposed");
    let state: SurfaceState;
    try {
      state = validateSurface(input);
    } catch {
      this.clear();
      throw new RendererError("renderer_invalid");
    }
    this.interaction = interaction && structuredClone(interaction);
    this.connected = connected;
    const enabled =
      connected &&
      this.interaction?.generation === state.generation &&
      this.pendingInteraction();
    const metadata = this.pending?.metadata;
    const pending =
      metadata &&
      metadata.sessionId === state.namespace.sessionId &&
      metadata.surfaceInstanceId === state.surfaceInstanceId &&
      metadata.surfaceRevision === state.revision &&
      metadata.generation === state.generation &&
      state.status === "active" &&
      this.interaction?.generation === state.generation &&
      (this.pendingInteraction() ||
        (this.interaction?.status === "answered" &&
          this.interaction.responseCommandId === metadata.commandId))
        ? this.pending
        : undefined;
    this.clear();
    this.pending = pending;
    const processor = new MessageProcessor(
      [catalog],
      async (message) => {
        if (
          this.disposed ||
          this.processor !== processor ||
          !enabled ||
          !this.pendingInteraction() ||
          this.sending
        )
          return;
        if (this.pending) {
          await this.retry();
          return;
        }
        const now = this.now();
        this.pending = {
          schemaVersion: 5,
          kind: "actionRequest",
          metadata: {
            schemaVersion: 5,
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
          expiresAtMs: Math.min(now + 30_000, this.interaction!.expiresAtMs),
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
        if (model) throw new RendererError("renderer_invalid");
        return;
      }
      if (!model || !model.componentsModel.get("root"))
        throw new RendererError("renderer_invalid");
      const element = document.createElement("a2ui-surface") as HTMLElement & {
        surface: typeof model;
      };
      element.surface = model;
      element.inert = !enabled;
      this.container.replaceChildren(element);
    } catch {
      this.clear();
      throw new RendererError("renderer_invalid");
    }
  }
  /** Retry uses exactly the same command ID, expiry and action payload. */
  async retry(): Promise<void> {
    if (!this.canRetry || this.sending) return;
    const request = this.pending!;
    this.sending = true;
    try {
      await this.options.onAction(structuredClone(request));
      if (this.pending === request) this.pending = undefined;
    } catch (error) {
      if (this.pending === request) {
        const failure =
          error instanceof ClientError ? error.code : "request_failed";
        const rejected = ![
          "request_failed",
          "transport_failed",
          "transport_closed",
          "revision_conflict",
        ].includes(failure);
        if (rejected) this.pending = undefined;
        try {
          this.options.onError(
            new RendererError(
              rejected ? "action_rejected" : "action_failed",
              failure,
            ),
          );
        } catch {
          /* observer exceptions cannot change action ownership */
        }
      }
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
  }
}
