import { randomUUID } from "node:crypto";
import { join } from "node:path";
import {
  boundedJson,
  decode,
  isId,
  type Binding,
  type Budget,
  type Command,
  type CommandRecord,
  type DispatchAttempt,
  type Failure,
  type Outcome,
  type ProviderAgentPort,
  type ProviderConfiguration,
  type ProviderObservation,
  type ProviderSessionBinding,
  type Reconciliation,
  type Result,
  type Submission,
} from "@rss-mdm-agent/ai-contract";
import {
  providerIdentity,
  VerifiedProviderSession,
  workspaceIdentity,
} from "@rss-mdm-agent/ai-contract/session";
import {
  ADAPTER_VERSION,
  compatible,
  launchSpec,
  type CodexAdapterOptions,
  type CodexConfiguration,
  type ResolvedCodexConfiguration,
} from "./configuration.js";
import {
  CODEX_VERSION,
  type NativeMessage,
  type RpcConnection,
  type RuntimeFactory,
} from "./runtime.js";
import { HOST_SERVER, HOST_TOOL, ToolBridge } from "./bridge.js";
import { rpc, type Thread, type Turn, type ThreadItem } from "./protocol.js";
import {
  bounded,
  copy,
  fail,
  limits,
  live,
  ok,
  Queue,
  same,
} from "./support.js";

export interface CodexDiagnostic {
  namespace: "codex.native";
  generation: string;
  message: NativeMessage;
}
export type CodexForkResult =
  | {
      certainty: "created";
      port: CodexAdapterPort;
      session: VerifiedProviderSession;
      source: { nativeThreadId: string; throughTurnId: string };
    }
  | {
      certainty: "not_created" | "unknown";
      error: Failure;
      correlationId?: string;
      cleanupError?: Failure;
      cleanupPort?: CodexAdapterPort;
    };
export interface CodexAdapterPort extends ProviderAgentPort {
  readHistory(
    binding: Binding,
    budget: Budget,
  ): Promise<Result<readonly Turn[]>>;
  fork(
    binding: Binding,
    throughTurnId: string,
    configuration: CodexConfiguration,
    budget: Budget,
  ): Promise<CodexForkResult>;
  diagnostics(binding: Binding, budget: Budget): AsyncIterable<CodexDiagnostic>;
}
interface Attempt {
  command: Command;
  dispatch: DispatchAttempt;
  binding: Binding;
  confirmed: boolean;
  outcome?: Outcome;
  completedItems: Set<string>;
}
const scope = (budget: Budget) => {
  const deadline = Date.now() + budget.timeoutMs;
  return () => ({
    signal: budget.signal,
    timeoutMs: Math.max(0, deadline - Date.now()),
  });
};
const outcome = (turn: Turn): Outcome | undefined =>
  (
    ({
      completed: "completed",
      interrupted: "cancelled",
      failed: "failed",
    }) as const
  )[turn.status as "completed" | "interrupted" | "failed"];
function validTurn(value: any): asserts value is Turn {
  if (
    !value ||
    !isId(value.id) ||
    !["inProgress", "completed", "interrupted", "failed"].includes(
      value.status,
    ) ||
    !Array.isArray(value.items)
  )
    throw new Error("invalid native turn");
  for (const item of value.items)
    if (!item || !isId(item.id) || typeof item.type !== "string")
      throw new Error("invalid native item");
}

/** One native incarnation. Host owns durable commands, retries and verified recovery. */
export class CodexAdapter implements CodexAdapterPort {
  private started = false;
  private closed = false;
  private failed = false;
  private initialized = false;
  private connection?: RpcConnection;
  private bridge?: ToolBridge;
  private binding?: Binding;
  private configuration?: CodexConfiguration;
  private resolved?: ResolvedCodexConfiguration;
  private attempts = new Map<string, Attempt>();
  private observations = new Queue<ProviderObservation>();
  private nativeEvents = new Queue<CodexDiagnostic>();
  private uncorrelated: NativeMessage[] = [];
  private uncorrelatedBytes = 0;
  private retainedAttemptBytes = 0;
  private completedTurns = new Map<string, Outcome>();
  private creationAttempted = false;
  constructor(
    private readonly options: CodexAdapterOptions,
    private readonly runtime: RuntimeFactory,
    private readonly forkSource?: {
      binding: Binding;
      throughTurnId: string;
      nativeDirectory: string;
    },
  ) {}

  createSession(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    return this.initialize(configuration, budget);
  }
  resume(
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    if (!compatible(binding)) return Promise.resolve(fail("stale_binding"));
    return this.initialize(configuration, budget, copy(binding));
  }
  private async initialize(
    configuration: ProviderConfiguration,
    budget: Budget,
    previous?: Binding,
  ): Promise<Result<ProviderSessionBinding>> {
    if (this.started || this.closed || !live(budget))
      return fail("unavailable");
    this.started = true;
    const nextBudget = scope(budget);
    try {
      if (
        configuration.provider !== "codex" ||
        !["tools_disabled", "host_mediated"].includes(configuration.permissions)
      )
        return fail("invalid_input");
      if (
        configuration.permissions === "host_mediated" &&
        (process.platform !== "darwin" || process.arch !== "arm64")
      )
        return fail("unsupported_capability");
      const config = {
        ...configuration,
        namespace: copy(configuration.namespace),
        config: copy(configuration.config),
      } as CodexConfiguration;
      const source = previous ?? this.forkSource?.binding;
      if (
        source &&
        (!compatible(source) ||
          source.accountRef !== config.accountRef ||
          !same(source.config, config.config) ||
          source.workspaceId !== workspaceIdentity(config.workingDirectory))
      )
        return fail("stale_binding");
      const resolved = await bounded(
        this.options.resolveConfiguration(
          {
            namespace: copy(config.namespace),
            provider: config.provider,
            config: copy(config.config),
            accountRef: config.accountRef,
            ...(source ? { history: copy(source) } : {}),
          },
          nextBudget(),
        ),
        nextBudget(),
      );
      const {
        tools: expectedTools,
        verifier: expectedVerifier,
        ...expected
      } = config;
      const { tools, verifier, ...actual } = resolved.configuration;
      if (
        !same(expected, actual) ||
        tools !== expectedTools ||
        verifier !== expectedVerifier
      )
        return fail("permission_denied");
      if (
        source &&
        !same(resolved.ownedHistory, {
          nativeSessionId: source.nativeSessionId,
          nativeThreadId: source.nativeThreadId,
        })
      )
        return fail("permission_denied");
      if (
        this.forkSource &&
        resolved.nativeDirectory !== this.forkSource.nativeDirectory
      )
        return fail("permission_denied");
      this.configuration = config;
      this.resolved = { ...resolved, configuration: config };
      if (this.closed || !live(nextBudget())) return fail("unavailable");
      if (config.permissions === "host_mediated") {
        this.bridge = new ToolBridge(
          config.tools,
          () =>
            this.initialized &&
            !this.closed &&
            !this.failed &&
            [...this.attempts.values()].some(
              (a) =>
                a.confirmed &&
                !a.outcome &&
                a.command.input.type === "prompt" &&
                a.command.input.policy === "queue_next",
            ),
        );
        await this.bridge.start(nextBudget());
      }
      const launch = await launchSpec(this.resolved, this.bridge);
      if (this.closed || !live(nextBudget())) return fail("unavailable");
      const connection = (this.connection = this.runtime(launch.spec));
      connection.listen((message) => {
        try {
          this.onNative(message);
        } catch {
          this.breakIncarnation();
        }
      });
      void connection.stopped.then(() => {
        if (!this.closed) this.breakIncarnation();
        this.observations.end();
        this.nativeEvents.end();
      });
      const handshake = await rpc(
        connection,
        "initialize",
        {
          clientInfo: {
            name: "rss_mdm_codex",
            title: "RSS Codex adapter",
            version: ADAPTER_VERSION,
          },
          capabilities: { experimentalApi: true, requestAttestation: false },
        },
        nextBudget(),
      );
      if (
        handshake.codexHome !== resolved.nativeDirectory ||
        typeof handshake.userAgent !== "string" ||
        !handshake.userAgent.includes(CODEX_VERSION)
      )
        return fail("unsupported_version");
      connection.notify("initialized");
      await this.checkConfiguration(
        launch.settings,
        launch.overrides,
        nextBudget(),
      );
      const params = {
        model: resolved.model,
        modelProvider: "rss_host_model",
        cwd: config.workingDirectory,
        approvalPolicy: "on-request" as const,
        sandbox: "read-only" as const,
        config: { ...launch.settings, ...launch.overrides } as any,
        runtimeWorkspaceRoots: [],
      };
      let response;
      if (previous) {
        response = await rpc(
          connection,
          "thread/resume",
          { ...params, threadId: previous.nativeThreadId!, excludeTurns: true },
          nextBudget(),
        );
      } else if (this.forkSource) {
        this.creationAttempted = true;
        response = await rpc(
          connection,
          "thread/fork",
          {
            ...params,
            threadId: this.forkSource.binding.nativeThreadId!,
            lastTurnId: this.forkSource.throughTurnId,
            excludeTurns: true,
          },
          nextBudget(),
        );
      } else {
        this.creationAttempted = true;
        response = await rpc(
          connection,
          "thread/start",
          { ...params, dynamicTools: [], environments: [], ephemeral: false },
          nextBudget(),
        );
      }
      const thread = response.thread;
      this.checkThread(thread, source !== undefined);
      if (
        previous &&
        (thread.id !== previous.nativeThreadId ||
          thread.sessionId !== previous.nativeSessionId)
      )
        return fail("stale_binding");
      if (
        this.forkSource &&
        (thread.id === this.forkSource.binding.nativeThreadId ||
          thread.forkedFromId !== this.forkSource.binding.nativeThreadId)
      )
        return fail("stale_binding");
      if (
        response.approvalPolicy !== "on-request" ||
        response.sandbox.type !== "readOnly" ||
        response.sandbox.networkAccess !== false ||
        !same(response.instructionSources, [])
      )
        return fail("permission_denied");
      this.binding = {
        provider: "codex",
        providerVersion: CODEX_VERSION,
        adapterVersion: ADAPTER_VERSION,
        generation: randomUUID(),
        workspaceId: workspaceIdentity(config.workingDirectory),
        accountRef: config.accountRef,
        config: copy(config.config),
        nativeSessionId: thread.sessionId,
        nativeThreadId: thread.id,
      };
      await this.checkConfiguration(
        launch.settings,
        launch.overrides,
        nextBudget(),
      );
      await this.checkMcp(nextBudget());
      if (this.closed || this.failed || !live(nextBudget()))
        return fail("unavailable");
      this.initialized = true;
      return ok({
        binding: copy(this.binding),
        capabilities: {
          queue: "unsupported",
          continuation: "across_processes",
          cancellation: "request_only",
          tools:
            config.permissions === "host_mediated"
              ? "host_mediated"
              : "disabled",
          steer: "supported",
          fork: "supported",
          subagent: "unsupported",
          terminal: "unsupported",
          structuredQuestion: "unsupported",
          multimodal: "unsupported",
        },
      });
    } catch {
      return fail("unavailable", "same_command");
    }
  }
  private async checkConfiguration(
    settings: Record<string, unknown>,
    overrides: Record<string, unknown>,
    budget: Budget,
  ): Promise<void> {
    const result = await rpc(
      this.connection!,
      "config/read",
      { includeLayers: true, cwd: this.configuration!.workingDirectory },
      budget,
    );
    if (!Array.isArray(result.layers))
      throw new Error("configuration layers unavailable");
    let user = 0,
      flags = 0;
    for (const layer of result.layers) {
      if (layer.disabledReason) throw new Error("disabled configuration layer");
      if (
        layer.name.type === "user" &&
        layer.name.file ===
          join(this.resolved!.nativeDirectory, "config.toml") &&
        layer.name.profile === null &&
        same(layer.config, settings)
      )
        user++;
      else if (
        layer.name.type === "sessionFlags" &&
        same(layer.config, overrides) &&
        Object.keys(overrides).length
      )
        flags++;
      else if (layer.name.type === "system" && same(layer.config, {})) {
        /* Empty system policy contributes no settings. */
      } else throw new Error("foreign configuration layer");
    }
    if (user !== 1 || flags !== (Object.keys(overrides).length ? 1 : 0))
      throw new Error("configuration lineage unavailable");
    const effective = result.config as any;
    if (
      effective.approval_policy !== "on-request" ||
      effective.sandbox_mode !== "read-only" ||
      effective.web_search !== "disabled"
    )
      throw new Error("unsafe native policy");
    const expected = (overrides.mcp_servers ?? settings.mcp_servers) as Record<
      string,
      unknown
    >;
    if (
      !same(
        Object.keys(effective.mcp_servers ?? {}).sort(),
        Object.keys(expected).sort(),
      )
    )
      throw new Error("foreign MCP endpoint");
    for (const [name, fields] of Object.entries(expected))
      for (const [key, value] of Object.entries(fields as object))
        if (!same(effective.mcp_servers[name]?.[key], value))
          throw new Error("MCP configuration changed");
  }
  private checkThread(thread: Thread, restored: boolean): void {
    // 0.155.0 resume/fork reconstitute the local environment even when the owned
    // source selected none. Every turn explicitly selects none again.
    const environments =
      same(thread?.environments, []) ||
      (restored &&
        same(thread?.environments, [
          {
            environmentId: "local",
            cwd: this.configuration!.workingDirectory,
            runtimeWorkspaceRoots: [],
          },
        ]));
    if (
      !thread ||
      !isId(thread.id) ||
      !isId(thread.sessionId) ||
      thread.parentThreadId !== null ||
      thread.cwd !== this.configuration!.workingDirectory ||
      thread.cliVersion !== CODEX_VERSION ||
      !environments
    )
      throw new Error("foreign native thread");
  }
  private async checkMcp(budget: Budget): Promise<void> {
    const nextBudget = scope(budget),
      servers: any[] = [],
      cursors = new Set<string>();
    let cursor: string | undefined;
    do {
      const page = await rpc(
        this.connection!,
        "mcpServerStatus/list",
        {
          threadId: this.binding!.nativeThreadId!,
          detail: "full",
          limit: 32,
          ...(cursor ? { cursor } : {}),
        },
        nextBudget(),
      );
      if (
        !Array.isArray(page.data) ||
        page.data.length > 32 ||
        servers.length + page.data.length > 32
      )
        throw new Error("MCP inventory limit");
      servers.push(...page.data);
      cursor = page.nextCursor ?? undefined;
      if (cursor) {
        if (cursors.has(cursor) || cursors.size >= 8)
          throw new Error("MCP cursor limit");
        cursors.add(cursor);
      }
    } while (cursor);
    if (!this.bridge) {
      if (servers.length) throw new Error("unexpected tools");
      return;
    }
    const server = servers[0];
    if (
      servers.length !== 1 ||
      server.name !== HOST_SERVER ||
      server.runtimeStatus !== "connected" ||
      server.toolsError !== null ||
      !same(Object.keys(server.tools), [HOST_TOOL.name]) ||
      !same(server.tools.propose.inputSchema, HOST_TOOL.inputSchema) ||
      !same(server.resources, []) ||
      !same(server.resourceTemplates, [])
    )
      throw new Error("unverified MCP catalog");
  }
  private owns(binding: Binding): boolean {
    return (
      !!this.binding &&
      same(providerIdentity(binding), providerIdentity(this.binding))
    );
  }
  private ready(binding: Binding, budget: Budget): boolean {
    return (
      this.initialized &&
      !this.closed &&
      !this.failed &&
      live(budget) &&
      this.owns(binding)
    );
  }
  private checked(command: Command): Command {
    const value = decode(boundedJson(command, limits), limits);
    if (
      value.kind !== "command" ||
      value.sessionId !== this.configuration!.namespace.sessionId ||
      value.expiresAtMs < (this.options.clock?.now() ?? Date.now())
    )
      throw new Error("invalid command");
    return value;
  }
  async submit(
    binding: Binding,
    command: Command,
    attempt: DispatchAttempt,
    budget: Budget,
  ): Promise<Submission> {
    const rejected = (code: Failure["code"]): Submission => ({
      certainty: "not_sent",
      error: { code, retry: "never" },
    });
    if (!this.ready(binding, budget)) return rejected("stale_binding");
    let input: Command;
    try {
      input = this.checked(command);
    } catch {
      return rejected("invalid_input");
    }
    if (input.input.type !== "prompt")
      return rejected("unsupported_capability");
    if (
      !isId(attempt.attemptId) ||
      attempt.certainty !== "intent" ||
      attempt.originGeneration !== binding.generation ||
      attempt.observerGeneration !== binding.generation ||
      attempt.nativeSessionId !== binding.nativeSessionId ||
      attempt.nativeThreadId !== binding.nativeThreadId
    )
      return rejected("stale_binding");
    const existing = this.attempts.get(input.commandId);
    if (existing) {
      if (
        !same(existing.command, input) ||
        existing.dispatch.attemptId !== attempt.attemptId
      )
        return rejected("content_conflict");
      return existing.confirmed
        ? { certainty: "submitted", binding: copy(existing.binding) }
        : { certainty: "unknown", correlationId: attempt.attemptId };
    }
    if (
      this.attempts.size >= 1024 ||
      this.retainedAttemptBytes +
        Buffer.byteLength(boundedJson(input, limits)) >
        4 * 1024 * 1024 ||
      [...this.attempts.values()].some(
        (a) => a.dispatch.attemptId === attempt.attemptId,
      )
    )
      return rejected("limit_exceeded");
    const unsettled = [...this.attempts.values()].filter((a) => !a.outcome);
    const steering = input.input.policy === "steer";
    if (steering) {
      if (
        attempt.nativeRunId !== input.input.targetRunId ||
        !unsettled.some(
          (a) =>
            a.confirmed &&
            a.binding.nativeRunId === input.input.targetRunId &&
            a.command.input.type === "prompt" &&
            a.command.input.policy === "queue_next",
        )
      )
        return rejected("stale_binding");
    } else if (unsettled.length || attempt.nativeRunId !== undefined)
      return rejected("reconciliation_required");
    const entry: Attempt = {
      command: copy(input),
      dispatch: copy(attempt),
      binding: {
        ...this.binding!,
        ...(steering ? { nativeRunId: input.input.targetRunId } : {}),
      },
      confirmed: false,
      completedItems: new Set(),
    };
    this.attempts.set(input.commandId, entry);
    this.retainedAttemptBytes += Buffer.byteLength(boundedJson(input, limits));
    let acknowledged = false;
    try {
      const prompt = [
        { type: "text" as const, text: input.input.text, text_elements: [] },
      ];
      if (steering) {
        const result = await rpc(
          this.connection!,
          "turn/steer",
          {
            threadId: binding.nativeThreadId!,
            expectedTurnId: input.input.targetRunId!,
            clientUserMessageId: attempt.attemptId,
            input: prompt,
          },
          budget,
        );
        acknowledged = true;
        if (result.turnId !== input.input.targetRunId)
          throw new Error("unexpected steer turn");
        this.confirm(entry, result.turnId);
      } else {
        const result = await rpc(
          this.connection!,
          "turn/start",
          {
            threadId: binding.nativeThreadId!,
            clientUserMessageId: attempt.attemptId,
            input: prompt,
            environments: [],
            runtimeWorkspaceRoots: [],
          },
          budget,
        );
        acknowledged = true;
        validTurn(result.turn);
        this.confirm(entry, result.turn.id);
        if (outcome(result.turn)) this.finishTurn(result.turn);
      }
      this.replay();
      return { certainty: "submitted", binding: copy(entry.binding) };
    } catch {
      if (acknowledged) this.breakIncarnation();
      return entry.confirmed
        ? { certainty: "submitted", binding: copy(entry.binding) }
        : { certainty: "unknown", correlationId: attempt.attemptId };
    }
  }
  private confirm(entry: Attempt, turnId: string, announce = true): void {
    if (
      !isId(turnId) ||
      (entry.binding.nativeRunId && entry.binding.nativeRunId !== turnId)
    )
      throw new Error("changed turn identity");
    entry.binding.nativeRunId = turnId;
    entry.binding.nativeRequestId = entry.dispatch.attemptId;
    if (!entry.confirmed) {
      entry.confirmed = true;
      if (announce)
        this.observations.push({
          type: "submitted",
          binding: copy(entry.binding),
          commandId: entry.command.commandId,
          attemptId: entry.dispatch.attemptId,
        });
    }
    const terminal = this.completedTurns.get(turnId);
    if (announce && terminal && !entry.outcome) {
      this.emit(entry, { type: "terminal", outcome: terminal });
      entry.outcome = terminal;
      entry.completedItems.clear();
    }
  }
  private emit(
    entry: Attempt,
    body: Extract<ProviderObservation, { type: "event" }>["body"],
  ): void {
    // Validate the stable product boundary; native advanced fields belong only to diagnostics.
    decode(
      boundedJson(
        {
          schemaVersion: 2,
          kind: "event",
          namespace: this.configuration!.namespace,
          eventId: "validation",
          sequence: 1,
          generation: entry.binding.generation,
          commandId: entry.command.commandId,
          attemptId: entry.dispatch.attemptId,
          body,
        },
        limits,
      ),
      limits,
    );
    this.observations.push({
      type: "event",
      binding: copy(entry.binding),
      commandId: entry.command.commandId,
      attemptId: entry.dispatch.attemptId,
      body,
    });
  }
  private onNative(message: NativeMessage, replay = false): void {
    if (this.closed) return;
    if (message.id !== undefined) {
      this.connection!.reject(message.id);
      return;
    }
    if (this.options.nativeDiagnostics && this.binding && !replay)
      this.nativeEvents.push({
        namespace: "codex.native",
        generation: this.binding.generation,
        message,
      });
    if (!this.binding || this.failed) return;
    const params = message.params as any;
    if (!params || params.threadId !== this.binding.nativeThreadId) return;
    const turnId = params.turnId ?? params.turn?.id;
    if (!isId(turnId)) return;
    const item: ThreadItem | undefined = params.item;
    if (item?.type === "userMessage" && typeof item.clientId === "string") {
      const entry = [...this.attempts.values()].find(
        (a) => a.dispatch.attemptId === item.clientId,
      );
      if (entry) {
        this.confirm(entry, turnId);
        if (!replay) this.replay();
      }
    }
    const owner = [...this.attempts.values()].find(
      (a) =>
        a.confirmed &&
        a.binding.nativeRunId === turnId &&
        a.command.input.type === "prompt" &&
        a.command.input.policy === "queue_next",
    );
    if (!owner) {
      if (
        [
          "item/agentMessage/delta",
          "item/completed",
          "item/started",
          "turn/completed",
        ].includes(message.method)
      ) {
        const bytes = Buffer.byteLength(boundedJson(message, limits));
        if (
          this.uncorrelated.length >= 128 ||
          this.uncorrelatedBytes + bytes > 4 * 1024 * 1024
        )
          throw new Error("uncorrelated event limit");
        this.uncorrelated.push(copy(message));
        this.uncorrelatedBytes += bytes;
      }
      return;
    }
    if (message.method === "turn/completed") {
      validTurn(params.turn);
      this.finishTurn(params.turn);
      return;
    }
    if (owner.outcome) return;
    if (message.method === "item/agentMessage/delta") {
      if (
        !isId(params.itemId) ||
        typeof params.delta !== "string" ||
        Buffer.byteLength(params.delta) > 65536
      )
        throw new Error("invalid delta");
      this.observations.push({
        type: "delta",
        binding: copy(owner.binding),
        commandId: owner.command.commandId,
        attemptId: owner.dispatch.attemptId,
        messageId: params.itemId,
        text: params.delta,
      });
    } else if (message.method === "item/completed" && item)
      this.completeItem(owner, item);
  }
  private completeItem(entry: Attempt, item: ThreadItem): void {
    if (!isId(item.id) || entry.completedItems.has(item.id)) return;
    if (entry.completedItems.size >= 8192) throw new Error("turn item limit");
    if (item.type === "agentMessage")
      this.emit(entry, { type: "text", messageId: item.id, text: item.text });
    else if (item.type === "mcpToolCall") {
      const args = item.arguments as any;
      if (
        item.server === HOST_SERVER &&
        item.tool === HOST_TOOL.name &&
        args &&
        isId(args.name) &&
        args.arguments &&
        typeof args.arguments === "object"
      ) {
        this.emit(entry, {
          type: "tool_proposal",
          proposalId: item.id,
          name: args.name,
          arguments: args.arguments,
        });
        const result = (item.result?.structuredContent as any)?.rssHostResult;
        const known =
          result &&
          same(Object.keys(result).sort(), ["disposition", "text"]) &&
          ["returned", "rejected", "unavailable"].includes(
            result.disposition,
          ) &&
          typeof result.text === "string";
        this.emit(entry, {
          type: "tool_result",
          proposalId: item.id,
          disposition: known ? result.disposition : "unavailable",
          text: known ? result.text : "Host proposal unavailable",
        });
      }
    }
    entry.completedItems.add(item.id);
  }
  private finishTurn(turn: Turn): void {
    const terminal = outcome(turn);
    if (!terminal) return;
    this.completedTurns.set(turn.id, terminal);
    for (const entry of this.attempts.values()) {
      if (
        entry.binding.nativeRunId !== turn.id ||
        !entry.confirmed ||
        entry.outcome
      )
        continue;
      if (
        entry.command.input.type === "prompt" &&
        entry.command.input.policy === "queue_next"
      )
        for (const item of turn.items) this.completeItem(entry, item);
      this.emit(entry, { type: "terminal", outcome: terminal });
      entry.outcome = terminal;
      entry.completedItems.clear();
    }
  }
  private replay(): void {
    for (let remaining = this.uncorrelated.length; remaining > 0; remaining--) {
      const message = this.uncorrelated.shift()!;
      this.uncorrelatedBytes -= Buffer.byteLength(boundedJson(message, limits));
      this.onNative(message, true);
    }
  }
  private breakIncarnation(): void {
    if (this.failed) return;
    this.failed = true;
    for (const entry of this.attempts.values()) {
      if (entry.outcome) continue;
      try {
        this.emit(entry, {
          type: "error",
          failure: { code: "unavailable", retry: "reconcile_first" },
        });
      } catch {
        break;
      }
    }
    this.observations.end();
    this.nativeEvents.end();
    void this.connection?.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
  }
  async *observe(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    if (this.owns(binding)) yield* this.observations.read(budget);
  }
  async *diagnostics(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<CodexDiagnostic> {
    if (this.options.nativeDiagnostics && this.owns(binding))
      yield* this.nativeEvents.read(budget);
  }
  async cancel(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<"request_only" | "already_terminal" | "unsupported">> {
    if (!this.ready(binding, budget)) return fail("stale_binding");
    let input: Command;
    try {
      input = this.checked(command);
    } catch {
      return fail("invalid_input");
    }
    if (
      input.input.type !== "cancel" ||
      input.input.generation !== binding.generation
    )
      return fail("stale_binding");
    const target = this.attempts.get(input.input.targetCommandId);
    if (
      !target ||
      (input.input.nativeRunId !== undefined &&
        input.input.nativeRunId !== target.binding.nativeRunId)
    )
      return fail("stale_binding");
    if (target.outcome) return ok("already_terminal");
    if (!target.confirmed || !target.binding.nativeRunId)
      return fail("reconciliation_required", "reconcile_first");
    try {
      await rpc(
        this.connection!,
        "turn/interrupt",
        {
          threadId: binding.nativeThreadId!,
          turnId: target.binding.nativeRunId,
        },
        budget,
      );
      return ok("request_only");
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  async respond(
    _binding: Binding,
    _command: Command,
    _budget: Budget,
  ): Promise<Result<void>> {
    return fail("unsupported_capability");
  }
  async readHistory(
    binding: Binding,
    budget: Budget,
  ): Promise<Result<readonly Turn[]>> {
    if (!this.ready(binding, budget)) return fail("stale_binding");
    try {
      const nextBudget = scope(budget),
        turns: Turn[] = [],
        cursors = new Set<string>();
      let cursor: string | undefined,
        bytes = 0;
      do {
        const page = await rpc(
          this.connection!,
          "thread/turns/list",
          {
            threadId: binding.nativeThreadId!,
            limit: 32,
            itemsView: "full",
            sortDirection: "asc",
            ...(cursor ? { cursor } : {}),
          },
          nextBudget(),
        );
        if (
          !Array.isArray(page.data) ||
          page.data.length > 32 ||
          turns.length + page.data.length > 1024
        )
          return fail("limit_exceeded");
        for (const turn of page.data) {
          validTurn(turn);
          if (turn.itemsView !== "full")
            throw new Error("incomplete native history");
          bytes += Buffer.byteLength(boundedJson(turn, limits));
        }
        if (bytes > 4 * 1024 * 1024) return fail("limit_exceeded");
        turns.push(...page.data);
        cursor = page.nextCursor ?? undefined;
        if (cursor) {
          if (cursors.has(cursor) || cursors.size >= 64)
            throw new Error("history cursor limit");
          cursors.add(cursor);
        }
      } while (cursor);
      return ok(turns.map(copy));
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  async reconcile(
    binding: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<Result<Reconciliation>> {
    if (!this.ready(binding, budget)) return fail("stale_binding");
    const dispatch = record.dispatch;
    if (
      !dispatch ||
      dispatch.nativeSessionId !== binding.nativeSessionId ||
      dispatch.nativeThreadId !== binding.nativeThreadId ||
      dispatch.observerGeneration !== binding.generation ||
      !same(record.receipt.namespace, this.configuration!.namespace) ||
      record.command.sessionId !== this.configuration!.namespace.sessionId ||
      !isId(dispatch.attemptId)
    )
      return fail("stale_binding");
    const history = await this.readHistory(binding, budget);
    if (!history.ok) return history;
    const matches = history.value.filter((turn) =>
      turn.items.some(
        (item) =>
          item.type === "userMessage" && item.clientId === dispatch.attemptId,
      ),
    );
    if (matches.length > 1) return fail("content_conflict");
    const found = matches[0];
    const observedBinding: Binding = {
      ...this.binding!,
      ...(dispatch.nativeRunId ? { nativeRunId: dispatch.nativeRunId } : {}),
      ...(dispatch.nativeRequestId
        ? { nativeRequestId: dispatch.nativeRequestId }
        : {}),
    };
    const base = {
      commandId: record.command.commandId,
      attemptId: dispatch.attemptId,
      binding: observedBinding,
    };
    if (!found) return ok({ ...base, status: "unknown" });
    if (
      (dispatch.nativeRunId && dispatch.nativeRunId !== found.id) ||
      (dispatch.nativeRequestId &&
        dispatch.nativeRequestId !== dispatch.attemptId)
    )
      return fail("stale_binding");
    const entry = this.attempts.get(record.command.commandId) ?? {
      command: copy(record.command),
      dispatch: copy(dispatch),
      binding: { ...observedBinding, nativeRequestId: dispatch.attemptId },
      confirmed: false,
      completedItems: new Set<string>(),
    };
    if (
      entry.dispatch.attemptId !== dispatch.attemptId ||
      !same(entry.command, record.command)
    )
      return fail("content_conflict");
    if (!this.attempts.has(record.command.commandId)) {
      const bytes = Buffer.byteLength(boundedJson(record.command, limits));
      if (
        this.attempts.size >= 1024 ||
        this.retainedAttemptBytes + bytes > 4 * 1024 * 1024
      )
        return fail("limit_exceeded");
      this.retainedAttemptBytes += bytes;
    }
    try {
      this.attempts.set(record.command.commandId, entry);
      this.confirm(entry, found.id, false);
      const terminal = outcome(found);
      // The Host commits the nominal reconciliation proof; recovery does not replay stable history.
      if (terminal) {
        entry.outcome = terminal;
        entry.completedItems.clear();
      }
      this.replay();
      return terminal
        ? ok({
            ...base,
            binding: copy(entry.binding),
            status: "terminal",
            outcome: terminal,
          })
        : ok({ ...base, binding: copy(entry.binding), status: "running" });
    } catch {
      this.breakIncarnation();
      return fail("unavailable", "reconcile_first");
    }
  }
  async fork(
    binding: Binding,
    throughTurnId: string,
    configuration: CodexConfiguration,
    budget: Budget,
  ): Promise<CodexForkResult> {
    const denied: CodexForkResult = {
      certainty: "not_created",
      error: { code: "permission_denied", retry: "never" },
    };
    if (!this.ready(binding, budget) || !isId(throughTurnId)) return denied;
    const { sessionId: parentId, ...parentScope } =
      this.configuration!.namespace;
    const { sessionId: childId, ...childScope } = configuration.namespace;
    if (
      parentId === childId ||
      !same(parentScope, childScope) ||
      !same(configuration.config, binding.config) ||
      configuration.accountRef !== binding.accountRef ||
      workspaceIdentity(configuration.workingDirectory) !== binding.workspaceId
    )
      return denied;
    const nextBudget = scope(budget),
      history = await this.readHistory(binding, nextBudget());
    if (!history.ok) return { certainty: "not_created", error: history.error };
    if (
      !history.value.some((turn) => turn.id === throughTurnId && outcome(turn))
    )
      return denied;
    const child = new CodexAdapter(this.options, this.runtime, {
      binding: copy(binding),
      throughTurnId,
      nativeDirectory: this.resolved!.nativeDirectory,
    });
    const admitted = await VerifiedProviderSession.open(
      child,
      configuration,
      nextBudget(),
    );
    if (admitted.ok)
      return {
        certainty: "created",
        port: child,
        session: admitted.value,
        source: { nativeThreadId: binding.nativeThreadId!, throughTurnId },
      };
    return {
      certainty: child.creationAttempted ? "unknown" : "not_created",
      error: admitted.error,
      ...(child.creationAttempted ? { correlationId: randomUUID() } : {}),
      ...(admitted.cleanupError
        ? { cleanupError: admitted.cleanupError, cleanupPort: child }
        : {}),
    };
  }
  async close(budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    this.closed = true;
    this.initialized = false;
    this.bridge?.quiesce();
    this.observations.end();
    this.nativeEvents.end();
    const nextBudget = scope(budget);
    const stopped = this.connection
      ? await this.connection.close(nextBudget())
      : ok({ processStopped: true });
    try {
      await this.bridge?.close(nextBudget());
    } catch {
      return fail("unavailable", "same_command");
    }
    return stopped;
  }
}
