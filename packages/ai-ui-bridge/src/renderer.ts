import { ClientError, type InteractionView } from "@rss-mdm-agent/ai-client";
import { RendererError } from "./errors.js";
import type { NodeMount } from "./operations/surface.js";
import {
  Catalog,
  MessageProcessor,
  NodeResolver,
  getValue,
  isComponentNode,
  isWritable,
  ResolvedBinding,
  type ComponentNode,
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
} from "@rss-mdm-agent/ai-contract";
import type {
  ActionRequest,
  SurfaceState,
} from "@rss-mdm-agent/ai-contract/wire";

import {
  SurfaceDrafts,
  SurfaceMemory,
  type NodeGate,
} from "./surface-drafts.js";

const catalog = new Catalog(
  interactionCatalog.catalogId,
  [A2uiText, A2uiColumn, A2uiButton, A2uiTextField],
  [],
);
export interface RendererOptions {
  onAction(request: ActionRequest): Promise<void>;
  onError(error: RendererError): void;
  onChange?(): void;
  now?: () => number;
  mount: NodeMount;
  drafts: SurfaceDrafts;
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
  private view?: ReturnType<NodeMount>;
  private fieldStops: (() => void)[] = [];
  private memory?: SurfaceMemory;
  private surface?: SurfaceState;
  private protectedInput = false;
  private resolver?: NodeResolver;
  private processor?: MessageProcessor<typeof A2uiText>;
  private get pending() {
    return this.memory?.pending;
  }
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
    const request = this.pending,
      surface = this.surface;
    return (
      !!request &&
      !!surface &&
      !this.disposed &&
      !this.memory?.retired &&
      !this.memory?.sending &&
      this.memory?.revision === surface.revision &&
      this.connected &&
      surface.status === "active" &&
      !this.protectedInput &&
      request.metadata.sessionId === surface.namespace.sessionId &&
      request.metadata.surfaceInstanceId === surface.surfaceInstanceId &&
      request.metadata.generation === surface.generation &&
      this.interaction?.generation === surface.generation &&
      request.metadata.surfaceRevision === surface.revision &&
      this.now() <= request.expiresAtMs &&
      (this.pendingInteraction() ||
        (this.interaction?.status === "answered" &&
          this.interaction.responseCommandId === request.metadata.commandId))
    );
  }
  private gate(memory: SurfaceMemory): NodeGate {
    return {
      snapshot: memory.snapshot,
      subscribe: memory.subscribe,
      canEdit: () =>
        !this.disposed &&
        !memory.retired &&
        this.memory === memory &&
        this.connected &&
        this.surface?.revision === memory.revision &&
        this.surface.status === "active" &&
        this.interaction?.generation === this.surface.generation &&
        this.pendingInteraction() &&
        !memory.pending &&
        !memory.sending &&
        !memory.rejected &&
        !this.protectedInput,
      reason: () =>
        memory.retired
          ? "当前身份已失效。"
          : this.surface?.revision !== memory.revision
            ? "问题版本已变化，请读取原会话。"
            : this.protectedInput
              ? "受保护输入不能在此问题表单中采集，请使用受控设置入口。"
              : memory.sending
                ? "正在提交原回答。"
                : memory.pending
                  ? this.surface?.revision !==
                      memory.pending.metadata.surfaceRevision ||
                    this.interaction?.generation !==
                      memory.pending.metadata.generation
                    ? "问题版本已变化，原回答仍待核实；请读取原会话。"
                    : this.interaction?.status === "answered"
                      ? "原回答已被记录，尚待核对原回执。"
                      : "已保留这份原回答，当前输入只读。"
                  : memory.notice,
    };
  }
  private releaseModel() {
    for (const stop of this.fieldStops) stop();
    this.fieldStops = [];
    this.resolver?.dispose();
    this.resolver = undefined;
    this.processor?.model.dispose();
    this.processor = undefined;
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
    this.surface = state;
    this.memory = this.options.drafts.acquire(state);
    const memory = this.memory;
    if (
      state.status === "deleted" ||
      ["expired", "unavailable"].includes(interaction?.status ?? "") ||
      (interaction?.status === "answered" &&
        interaction.responseCommandId !== memory.pending?.metadata.commandId)
    ) {
      memory.pending = undefined;
    }
    // Only renderer resources are replaced. Input and uncertain answer identity stay in the application.
    this.releaseModel();
    this.protectedInput = false;
    const gate = this.gate(memory);
    const enabled =
      connected &&
      interaction?.generation === state.generation &&
      this.pendingInteraction();
    const processor = new MessageProcessor(
      [catalog],
      async (message) => {
        if (
          this.disposed ||
          this.processor !== processor ||
          !enabled ||
          !this.pendingInteraction() ||
          memory.sending ||
          memory.retired ||
          memory.rejected ||
          memory.revision !== state.revision ||
          !this.connected ||
          this.protectedInput
        )
          return;
        if (memory.pending) return;
        const now = this.now();
        memory.pending = {
          schemaVersion: 1,
          kind: "actionRequest",
          metadata: {
            schemaVersion: 1,
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
        memory.changed();
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
        this.view?.render(undefined, false, gate);
        memory.changed();
        return;
      }
      if (!model || !model.componentsModel.get("root"))
        throw new RendererError("renderer_invalid");
      this.resolver = new NodeResolver(model, catalog);
      const root = getValue(this.resolver.rootNode);
      if (!root) throw new RendererError("renderer_invalid");
      const modelFields = new Map<
        string,
        {
          path: string;
          binding: import("@a2ui/web_core/v0_9").WritableBinding<unknown>;
        }
      >();
      const walk = (node: ComponentNode) => {
        const props = getValue(node.props);
        if (node.type === "TextField") {
          const variant =
            props.variant instanceof ResolvedBinding
              ? props.variant.value
              : props.variant;
          if (variant === "obscured") this.protectedInput = true;
          const binding = props.value;
          if (
            variant !== "obscured" &&
            binding instanceof ResolvedBinding &&
            isWritable(binding)
          ) {
            const key = JSON.stringify([
              node.componentId,
              node.dataPath,
              binding.path,
              variant ?? "shortText",
            ]);
            const path = binding.path.startsWith("/")
              ? binding.path
              : `${node.dataPath.replace(/\/$/, "")}/${binding.path}`;
            modelFields.set(key, { path, binding });
          }
        }
        for (const value of Object.values(props)) {
          if (isComponentNode(value)) walk(value);
          else if (Array.isArray(value))
            for (const child of value) if (isComponentNode(child)) walk(child);
        }
      };
      walk(root);
      if (memory.revision === state.revision)
        memory.retain(new Set(modelFields.keys()));
      for (const [key, field] of modelFields) {
        const prior = memory.fields.get(key);
        if (prior !== undefined) field.binding.set(prior);
      }
      for (const [key, field] of modelFields) {
        const subscription = model.dataModel.subscribe<unknown>(
          field.path,
          (value) => {
            if (
              this.disposed ||
              this.memory !== memory ||
              memory.retired ||
              memory.revision !== state.revision
            )
              return;
            if (typeof value === "string" && value.length <= 8192)
              memory.edit(key, value);
          },
        );
        this.fieldStops.push(() => subscription.unsubscribe());
      }
      this.fieldStops.push(
        memory.subscribe(() => {
          if (
            this.disposed ||
            this.memory !== memory ||
            memory.retired ||
            memory.revision !== state.revision
          )
            return;
          for (const [key, field] of modelFields) {
            const value = memory.fields.get(key);
            if (
              value !== undefined &&
              model.dataModel.get(field.path) !== value
            )
              field.binding.set(value);
          }
          this.options.onChange?.();
        }),
      );
      this.view ??= this.options.mount(this.container, () =>
        this.options.onError(new RendererError("renderer_invalid")),
      );
      this.view.render(root, enabled, gate);
      memory.changed();
    } catch {
      this.clear();
      throw new RendererError("renderer_invalid");
    }
  }
  /** Retry uses exactly the same command ID, expiry and action payload. */
  async retry(): Promise<void> {
    const memory = this.memory;
    if (!memory || !this.canRetry || memory.sending) return;
    const request = memory.pending!;
    memory.sending = true;
    memory.changed();
    try {
      await this.options.onAction(structuredClone(request));
      if (memory.pending === request) memory.pending = undefined;
    } catch (error) {
      if (memory.pending === request) {
        const failure =
          error instanceof ClientError ? error.code : "request_failed";
        const rejected = ![
          "request_failed",
          "transport_failed",
          "transport_closed",
          "revision_conflict",
        ].includes(failure);
        if (rejected) {
          memory.pending = undefined;
          if (
            [
              "already_answered",
              "expired",
              "unavailable",
              "stale_binding",
              "permission_denied",
            ].includes(failure)
          ) {
            memory.rejected = true;
            memory.notice = "原问题不能再接受此回答，请读取原会话。";
          }
        }
        try {
          this.options.onError(
            new RendererError(
              rejected ? "action_rejected" : "action_failed",
              failure,
            ),
          );
        } catch {
          /* observers cannot change answer ownership */
        }
      }
    } finally {
      memory.sending = false;
      memory.changed();
    }
  }
  clear(): void {
    this.connected = false;
    this.interaction = undefined;
    this.surface = undefined;
    this.releaseModel();
    this.view?.render(
      undefined,
      false,
      this.gate(this.memory ?? new SurfaceMemory()),
    );
    this.memory?.changed();
  }
  dispose(): void {
    this.disposed = true;
    this.releaseModel();
    this.view?.dispose();
    this.view = undefined;
    this.memory?.changed();
  }
}
