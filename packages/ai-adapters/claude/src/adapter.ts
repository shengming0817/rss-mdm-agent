import { randomUUID } from "node:crypto";
import {
  createSdkMcpServer,
  tool,
  type CanUseTool,
  type Options,
  type SDKMessage,
  type SDKUserMessage,
} from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";
import {
  decode,
  boundedJson,
  fingerprint,
  type Binding,
  type Budget,
  type Command,
  type CommandRecord,
  type Outcome,
  type ProviderAgentPort,
  type ProviderConfiguration,
  type ProviderObservation,
  type ProviderSessionBinding,
  type Result,
  type Submission,
} from "@rss-mdm-agent/ai-contract";
import {
  ADAPTER_VERSION,
  CLI_VERSION,
  PROVIDER_VERSION,
  sdkOptions,
  type ClaudeAdapterOptions,
} from "./configuration.js";
import { Interactions } from "./interactions.js";
import type { RuntimeFactory } from "./runtime.js";
import {
  bounded,
  ByteBudget,
  copy,
  deferred,
  fail,
  id,
  limits,
  liveBudget,
  ok,
  Queue,
  same,
} from "./support.js";
const bridge = "mcp__rss_host__propose";
interface Turn {
  command: Pick<Command, "sessionId" | "commandId">;
  hash: string;
  binding: Binding;
  inputBinding: Binding;
  queue: Queue<ProviderObservation>;
  acceptance: ReturnType<typeof deferred<boolean>>;
  consumed: boolean;
  accepted: boolean;
  outcome?: Outcome;
  uncertain: boolean;
  observing: boolean;
  streamMessageId?: string;
  interactions: Interactions;
}
interface Session {
  binding: Binding;
  configuration: ProviderConfiguration;
  input: Queue<SDKUserMessage>;
  runtime: ReturnType<RuntimeFactory>;
  abort: AbortController;
  turns: Map<string, Turn>;
  displayBudget: ByteBudget;
  active?: Turn;
  initialized: boolean;
  closing: boolean;
  stopped: boolean;
  failed: boolean;
}
export class ClaudeAdapter implements ProviderAgentPort {
  private session?: Session;
  private opening = false;
  private epoch = 0;
  private readonly now: () => number;
  private readonly ttl: number;
  constructor(
    private readonly options: ClaudeAdapterOptions,
    private readonly factory: RuntimeFactory,
  ) {
    this.now = () => options.clock?.now() ?? Date.now();
    this.ttl = options.callbackTimeoutMs ?? 120000;
    if (!Number.isSafeInteger(this.ttl) || this.ttl < 1 || this.ttl > 600000)
      throw new Error("invalid callback budget");
  }
  async createSession(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    return this.open(configuration, budget);
  }
  private async open(
    requested: ProviderConfiguration,
    budget: Budget,
    previous?: Binding,
  ): Promise<Result<ProviderSessionBinding>> {
    if (!liveBudget(budget)) return fail("unavailable", "same_command");
    const deadline = Date.now() + budget.timeoutMs;
    const remaining = (): Budget => ({
      ...budget,
      timeoutMs: deadline - Date.now(),
    });
    if (this.opening || (this.session && !this.session.stopped))
      return fail("unavailable");
    this.opening = true;
    const epoch = ++this.epoch;
    try {
      // Capture admission identities before calling external configuration code.
      const request = { ...requested, config: copy(requested.config) };
      const prior = previous ? copy(previous) : undefined;
      if (
        prior &&
        (prior.provider !== request.provider ||
          !same(prior.config, request.config) ||
          prior.accountRef !== request.accountRef)
      )
        return fail("stale_binding");
      const identity = copy({
        config: request.config,
        accountRef: request.accountRef,
      });
      const supplied = await bounded(
        this.options.resolveConfiguration(identity, budget),
        budget,
      );
      const config = {
        ...supplied.configuration,
        config: copy(supplied.configuration.config),
      };
      const resolved = {
        ...supplied,
        configuration: config,
        credential: { ...supplied.credential },
      };
      if (epoch !== this.epoch || !liveBudget(remaining()))
        return fail("unavailable");
      if (
        config.provider !== "claude" ||
        request.provider !== "claude" ||
        !same(config.config, identity.config) ||
        config.accountRef !== identity.accountRef
      )
        return fail("stale_binding");
      if (
        request.workingDirectory !== config.workingDirectory ||
        request.permissions !== config.permissions ||
        request.tools !== config.tools ||
        request.verifier !== config.verifier
      )
        return fail("permission_denied");
      if (
        config.permissions === "host_mediated"
          ? typeof config.tools?.propose !== "function" ||
            typeof config.verifier?.verify !== "function"
          : config.permissions !== "tools_disabled" ||
            config.tools !== undefined ||
            config.verifier !== undefined
      )
        return fail("permission_denied");
      const resume = prior !== undefined;
      if (
        resume &&
        (prior.providerVersion !== PROVIDER_VERSION ||
          prior.adapterVersion !== ADAPTER_VERSION ||
          !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
            prior.nativeSessionId,
          ))
      )
        return fail("unsupported_version");
      const binding: Binding = {
        provider: "claude",
        providerVersion: PROVIDER_VERSION,
        adapterVersion: ADAPTER_VERSION,
        config: identity.config,
        accountRef: identity.accountRef,
        generation: randomUUID(),
        nativeSessionId: resume ? prior.nativeSessionId : randomUUID(),
      };
      const input = new Queue<SDKUserMessage>(1),
        abort = new AbortController(),
        options: Options = {
          ...sdkOptions(resolved),
          abortController: abort,
          ...(resume
            ? { resume: binding.nativeSessionId }
            : { sessionId: binding.nativeSessionId }),
        };
      let session: Session;
      options.canUseTool = async (name, args, context) => {
        try {
          if (
            !session?.initialized ||
            session.closing ||
            session.failed ||
            context.signal.aborted ||
            !session.active?.accepted
          )
            return { behavior: "deny", message: "Tool unavailable" };
          const turn = session.active;
          if (
            name === "AskUserQuestion" &&
            !context.agentID &&
            !context.mcpServer
          )
            return await turn.interactions.ask(
              turn.binding,
              turn.command,
              args,
              context,
            );
          if (
            this.isBridge(session, name, context.mcpServer) &&
            !context.agentID
          )
            return { behavior: "allow", updatedInput: copy(args) };
        } catch {
          /* Value-free fail closed. */
        }
        return { behavior: "deny", message: "Tool unavailable" };
      };
      options.hooks = {
        PreToolUse: [
          {
            hooks: [
              async (input) => ({
                hookSpecificOutput: {
                  hookEventName: "PreToolUse",
                  permissionDecision:
                    input.hook_event_name === "PreToolUse" &&
                    session?.initialized &&
                    !session.closing &&
                    !session.failed &&
                    input.session_id === binding.nativeSessionId &&
                    ((input.tool_name === "AskUserQuestion" &&
                      !input.mcp_server) ||
                      this.isBridge(session, input.tool_name, input.mcp_server))
                      ? "ask"
                      : "deny",
                  permissionDecisionReason: "Host mediated tool policy",
                },
              }),
            ],
          },
        ],
        UserPromptExpansion: [
          {
            hooks: [
              async () => ({
                hookSpecificOutput: {
                  hookEventName: "UserPromptExpansion",
                  decision: "block",
                  reason: "Prompt expansion disabled",
                },
              }),
            ],
          },
        ],
      };
      if (config.permissions === "host_mediated")
        options.mcpServers = {
          rss_host: createSdkMcpServer({
            name: "rss_host",
            version: "1.0.0",
            alwaysLoad: true,
            tools: [
              tool(
                "propose",
                "Submit an untrusted business proposal to the Host. This never grants approval.",
                {
                  name: z
                    .string()
                    .regex(/^[A-Za-z0-9][A-Za-z0-9._:/+\-]{0,127}$/),
                  // SDK 0.3.277's bundled schema converter cannot list a Zod 4
                  // record here; an open object preserves the same JSON payload.
                  arguments: z.object({}).catchall(z.unknown()),
                },
                async (args) => {
                  const turn = session?.active;
                  if (
                    !session?.initialized ||
                    session.closing ||
                    session.failed ||
                    !turn?.consumed ||
                    turn.outcome
                  )
                    return {
                      isError: true,
                      content: [{ type: "text", text: "Tool unavailable" }],
                    };
                  const proposalId = randomUUID(),
                    budget: Budget = {
                      timeoutMs: 30000,
                      signal: AbortSignal.any([
                        session.abort.signal,
                        AbortSignal.timeout(30000),
                      ]),
                    };
                  try {
                    const proposal = copy(args);
                    this.emit(turn, {
                      type: "tool_proposal",
                      proposalId,
                      ...proposal,
                    });
                    const result = await bounded(
                      config.tools.propose(proposal, budget),
                      budget,
                    );
                    const value = result.ok
                      ? copy(result.value)
                      : {
                          disposition: "unavailable" as const,
                          text: "Tool unavailable",
                        };
                    if (
                      !["returned", "rejected", "unavailable"].includes(
                        value.disposition,
                      ) ||
                      typeof value.text !== "string"
                    )
                      throw new Error("invalid tool result");
                    if (
                      session.active !== turn ||
                      turn.outcome ||
                      session.closing
                    )
                      throw new Error("stale tool result");
                    this.emit(turn, {
                      type: "tool_result",
                      proposalId,
                      ...value,
                    });
                    return {
                      isError: value.disposition !== "returned",
                      content: [{ type: "text", text: value.text }],
                    };
                  } catch {
                    return {
                      isError: true,
                      content: [{ type: "text", text: "Tool unavailable" }],
                    };
                  }
                },
              ),
            ],
          }),
        };
      const runtime = this.factory(options, {
        async *[Symbol.asyncIterator]() {
          for await (const user of input) {
            if (session.active) session.active.consumed = true;
            yield user;
          }
        },
      });
      session = {
        binding,
        configuration: config,
        input,
        runtime,
        abort,
        turns: new Map(),
        displayBudget: new ByteBudget(4 * 1024 * 1024),
        initialized: false,
        closing: false,
        stopped: false,
        failed: false,
      };
      this.session = session;
      void runtime.stopped.then(
        () => {
          session.stopped = true;
          this.lost(session);
        },
        () => this.lost(session),
      );
      void this.pump(session);
      await bounded(runtime.query.initializationResult(), remaining());
      if (
        epoch !== this.epoch ||
        budget.signal.aborted ||
        session.failed ||
        session.closing
      )
        throw new Error("initialization failed");
      return ok({
        binding: copy(binding),
        capabilities: {
          continuation: "across_processes",
          cancellation: "request_only",
          tools:
            config.permissions === "host_mediated"
              ? "host_mediated"
              : "disabled",
          queue: "unsupported",
          steer: "unsupported",
          fork: "unsupported",
          subagent: "unsupported",
          terminal: "unsupported",
          structuredQuestion: "supported",
          multimodal: "unsupported",
        },
      });
    } catch {
      if (this.session && !this.session.stopped) this.stop(this.session);
      return fail("unavailable", "same_command");
    } finally {
      this.opening = false;
    }
  }
  private isBridge(
    session: Session,
    name: string,
    server?: { name: string; source: string },
  ): boolean {
    return (
      session.configuration.permissions === "host_mediated" &&
      name === bridge &&
      server?.name === "rss_host" &&
      server.source === "sdk"
    );
  }
  private checked(command: Command): Command {
    const value = decode(boundedJson(command, limits), limits);
    if (value.kind !== "command" || value.expiresAtMs <= this.now())
      throw new Error("invalid command");
    return value;
  }
  async submit(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Submission> {
    const denied = (
      code: Parameters<typeof fail>[0],
      retry: Parameters<typeof fail>[1] = "never",
    ): Submission => ({
      certainty: "not_sent",
      error: { code, retry: retry! },
    });
    let c: Command;
    try {
      c = this.checked(command);
    } catch {
      return denied("invalid_input");
    }
    const s = this.session;
    if (!s || s.closing || s.failed || !liveBudget(budget))
      return denied("unavailable", "same_command");
    const prior = s.turns.get(c.commandId);
    if (prior) {
      if (!same(binding, prior.inputBinding) && !same(binding, prior.binding))
        return denied("stale_binding");
      if (prior.hash !== fingerprint(c, limits))
        return denied("content_conflict");
      return prior.accepted
        ? { certainty: "submitted", binding: copy(prior.binding) }
        : { certainty: "unknown", correlationId: c.commandId };
    }
    if (!same(binding, s.binding)) return denied("stale_binding");
    if (c.input.type !== "prompt") return denied("invalid_input");
    if (c.input.policy === "steer") return denied("unsupported_capability");
    if (c.input.text.trimStart().startsWith("/"))
      return denied("unsupported_capability");
    if (s.active && !s.active.outcome)
      return denied("unavailable", "same_command");
    if (s.turns.size >= 256) return denied("limit_exceeded");
    const live = { ...copy(binding), nativeRequestId: randomUUID() };
    const turn: Turn = {
      command: { sessionId: c.sessionId, commandId: c.commandId },
      hash: fingerprint(c, limits),
      binding: live,
      inputBinding: copy(binding),
      queue: new Queue(1024, 2 * 1024 * 1024, s.displayBudget),
      acceptance: deferred<boolean>(),
      consumed: false,
      accepted: false,
      uncertain: false,
      observing: false,
      interactions: undefined!,
    };
    turn.interactions = new Interactions(this.now, this.ttl, (event) => {
      try {
        turn.queue.push(event);
      } catch {
        this.lost(s);
      }
    });
    s.active = turn;
    s.turns.set(c.commandId, turn);
    try {
      s.input.push({
        type: "user",
        session_id: binding.nativeSessionId,
        uuid: live.nativeRequestId as `${string}-${string}-${string}-${string}-${string}`,
        message: { role: "user", content: c.input.text },
        parent_tool_use_id: null,
      });
      const accepted = await bounded(turn.acceptance.promise, budget);
      if (accepted) return { certainty: "submitted", binding: copy(live) };
    } catch {
      /* Native transport ownership may already have started. */
    }
    turn.uncertain = true;
    return { certainty: "unknown", correlationId: c.commandId };
  }
  private emit(
    turn: Turn,
    body: Extract<ProviderObservation, { type: "event" }>["body"],
  ): void {
    turn.queue.push({
      type: "event",
      binding: copy(turn.binding),
      commandId: turn.command.commandId,
      body,
    });
  }
  private accept(s: Session, turn: Turn): void {
    if (!turn.accepted) {
      turn.accepted = true;
      turn.uncertain = false;
      s.binding = copy(turn.binding);
      turn.acceptance.resolve(true);
      this.emit(turn, { type: "status", state: "running" });
    }
  }
  private async pump(s: Session): Promise<void> {
    try {
      for await (const message of s.runtime.query) {
        if (s.closing) break;
        this.message(s, message);
      }
    } catch {
      /* SDK errors may contain secrets; only uncertainty crosses the port. */
    } finally {
      this.stop(s);
    }
  }
  private message(s: Session, m: SDKMessage): void {
    if (
      !(m.type === "system" && m.subtype === "init") &&
      !["user", "assistant", "stream_event", "result"].includes(m.type)
    )
      return;
    if (!("session_id" in m) || m.session_id !== s.binding.nativeSessionId)
      throw new Error("foreign session");
    if (m.type === "system" && m.subtype === "init") {
      const allowed = new Set([
        "AskUserQuestion",
        ...(s.configuration.permissions === "host_mediated" ? [bridge] : []),
      ]);
      if (
        m.claude_code_version !== CLI_VERSION ||
        m.permissionMode !== "default" ||
        m.tools.some((t) => !allowed.has(t)) ||
        [...allowed].some((t) => !m.tools.includes(t)) ||
        m.skills.length ||
        m.plugins.length ||
        (s.configuration.permissions === "host_mediated" &&
          m.mcp_servers.length !== 1) ||
        m.mcp_servers.some(
          (server) =>
            s.configuration.permissions !== "host_mediated" ||
            server.name !== "rss_host" ||
            server.source !== "sdk" ||
            server.status !== "connected",
        )
      )
        throw new Error("unverified runtime");
      s.initialized = true;
      return;
    }
    const turn = s.active;
    if (!turn || !turn.consumed || turn.outcome) return;
    if (!s.initialized) throw new Error("missing initialization");
    if ("parent_tool_use_id" in m && m.parent_tool_use_id) return;
    if (m.type === "user" && m.uuid === turn.binding.nativeRequestId) {
      this.accept(s, turn);
      return;
    }
    if (
      (m.type === "assistant" || m.type === "stream_event") &&
      m.user_message_uuid
    ) {
      if (m.user_message_uuid !== turn.binding.nativeRequestId)
        throw new Error("foreign prompt");
      this.accept(s, turn);
    }
    if (m.type === "assistant") {
      if (!turn.accepted) return;
      if (!id(m.message.id)) throw new Error("invalid message id");
      for (const part of m.message.content)
        if (part.type === "text")
          this.emit(turn, {
            type: "text",
            messageId: m.message.id,
            text: part.text,
          });
    } else if (m.type === "stream_event") {
      if (!turn.accepted) return;
      if (m.event.type === "message_start") {
        if (!id(m.event.message.id)) throw new Error("invalid message id");
        turn.streamMessageId = m.event.message.id;
      } else if (
        m.event.type === "content_block_delta" &&
        m.event.delta.type === "text_delta"
      ) {
        if (!turn.streamMessageId) throw new Error("missing message id");
        turn.queue.push({
          type: "delta",
          binding: copy(turn.binding),
          commandId: turn.command.commandId,
          messageId: turn.streamMessageId,
          text: m.event.delta.text,
        });
      } else if (m.event.type === "message_stop")
        turn.streamMessageId = undefined;
    } else if (m.type === "result") {
      if (m.user_message_uuid !== turn.binding.nativeRequestId) {
        turn.uncertain = true;
        turn.acceptance.resolve(false);
        turn.interactions.invalidate();
        turn.queue.end();
        return;
      }
      this.accept(s, turn);
      const outcome: Outcome =
        m.terminal_reason === "aborted_streaming" ||
        m.terminal_reason === "aborted_tools"
          ? "cancelled"
          : m.stop_reason === "refusal"
            ? "refused"
            : m.stop_reason === "max_tokens"
              ? "max_tokens"
              : m.subtype === "error_max_turns"
                ? "max_turn_requests"
                : m.subtype === "success" && !m.is_error
                  ? "completed"
                  : "failed";
      turn.interactions.invalidate();
      turn.outcome = outcome;
      this.emit(turn, { type: "terminal", outcome });
      turn.queue.end();
    }
  }
  private lost(s: Session): void {
    s.failed = true;
    for (const turn of s.turns.values()) {
      if (!turn.outcome && !turn.uncertain) {
        turn.uncertain = true;
        try {
          turn.queue.push({
            type: "event",
            binding: copy(turn.accepted ? turn.binding : turn.inputBinding),
            commandId: turn.command.commandId,
            body: {
              type: "error",
              failure: {
                code: "reconciliation_required",
                retry: "reconcile_first",
              },
            },
          });
        } catch {
          /* A full/closed display queue cannot manufacture terminal evidence. */
        }
      }
      turn.acceptance.resolve(false);
      turn.interactions.invalidate();
      turn.queue.end();
    }
  }
  private stop(s: Session): void {
    s.closing = true;
    this.lost(s);
    s.input.end();
    try {
      s.runtime.query.close();
    } catch {}
    s.abort.abort();
  }
  private turn(binding: Binding): Turn | undefined {
    return (
      this.session &&
      [...this.session.turns.values()].find(
        (t) =>
          same(binding, t.binding) ||
          (!t.accepted && same(binding, t.inputBinding)),
      )
    );
  }
  async cancel(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<"request_only" | "already_terminal">> {
    if (!liveBudget(budget)) return fail("unavailable", "same_command");
    let c: Command;
    try {
      c = this.checked(command);
    } catch {
      return fail("invalid_input");
    }
    const s = this.session,
      turn = this.turn(binding);
    if (
      !s ||
      !turn ||
      c.input.type !== "cancel" ||
      c.input.targetCommandId !== turn.command.commandId ||
      c.sessionId !== turn.command.sessionId ||
      c.input.generation !== binding.generation ||
      c.input.nativeRunId !== binding.nativeRunId
    )
      return fail("stale_binding");
    if (turn.outcome) return ok("already_terminal");
    if (s.active !== turn || s.closing || s.failed) return fail("unavailable");
    try {
      await bounded(s.runtime.query.interrupt(), budget);
      return ok("request_only");
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  async respond(
    binding: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<void>> {
    if (
      !liveBudget(budget) ||
      !this.session ||
      this.session.closing ||
      this.session.failed
    )
      return fail("unavailable");
    try {
      const c = this.checked(command);
      return (
        this.turn(binding)?.interactions.respond(binding, c) ??
        fail("stale_binding")
      );
    } catch {
      return fail("invalid_input");
    }
  }
  async *observe(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    const turn = this.turn(binding);
    if (!turn || turn.observing) return;
    turn.observing = true;
    try {
      for await (const item of turn.queue.read(budget)) yield item;
    } finally {
      turn.observing = false;
    }
  }
  async reconcile(
    binding: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<
    Result<{
      status: "running" | "terminal" | "not_submitted" | "unknown";
      binding: Binding;
      outcome?: Outcome;
    }>
  > {
    if (!liveBudget(budget)) return fail("unavailable", "same_command");
    const s = this.session;
    if (
      !s ||
      (!same(binding, s.binding) &&
        !this.turn(binding) &&
        !same(binding, s.turns.get(record.command.commandId)?.inputBinding))
    )
      return fail("stale_binding");
    const turn = s.turns.get(record.command.commandId);
    if (!turn) return ok({ status: "unknown", binding: copy(binding) });
    try {
      if (turn.hash !== fingerprint(record.command, limits))
        return fail("content_conflict");
    } catch {
      return fail("invalid_input");
    }
    return ok({
      status: turn.outcome
        ? "terminal"
        : turn.accepted && !turn.uncertain
          ? "running"
          : "unknown",
      binding: copy(turn.accepted ? turn.binding : binding),
      ...(turn.outcome ? { outcome: turn.outcome } : {}),
    });
  }
  async resume(
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    try {
      return await this.open(configuration, budget, copy(binding));
    } catch {
      return fail("invalid_input");
    }
  }
  async close(budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    ++this.epoch;
    const s = this.session;
    if (!s)
      return this.opening
        ? fail("unavailable", "same_command")
        : ok({ processStopped: true });
    if (!s.closing) this.stop(s);
    try {
      await bounded(s.runtime.stopped, budget);
      s.stopped = true;
      return ok({ processStopped: true });
    } catch {
      return fail("unavailable", "same_command");
    }
  }
}
