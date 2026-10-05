import { operationRequestId } from "@rss-mdm-agent/execution-bindings";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import {
  ReadBuffer,
  serializeMessage,
} from "@modelcontextprotocol/sdk/shared/stdio.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import type { JSONRPCMessage } from "@modelcontextprotocol/sdk/types.js";
import type { Readable, Writable } from "node:stream";
import { createHash, randomUUID } from "node:crypto";
import { boundedJson, isId } from "@rss-mdm-agent/ai-contract";
import type { Budget } from "@rss-mdm-agent/ai-contract/ports";
import type {
  ExecutionOrigin,
  Namespace,
} from "@rss-mdm-agent/ai-contract/wire";
import {
  defaultLimits,
  fail,
  ok,
} from "@rss-mdm-agent/ai-contract/transitions";
import type {
  DeliveryRouter,
  DeliveryRequest,
  DeliveryReceipt,
} from "@rss-mdm-agent/ai-host";

/** MCP over the parent's existing pipes. No child launcher and no second wire protocol. */
export class ParentTransport implements Transport {
  onclose?: Transport["onclose"];
  onerror?: Transport["onerror"];
  onmessage?: Transport["onmessage"];
  private readonly buffer = new ReadBuffer({ maxBufferSize: 262144 });
  private closed = false;
  private finish!: () => void;
  readonly stopped = new Promise<void>((resolve) => {
    this.finish = resolve;
  });
  constructor(
    private readonly input: Readable,
    private readonly output: Writable,
  ) {}
  private readonly receive = (chunk: Buffer) => {
    try {
      this.buffer.append(chunk);
      for (let message; (message = this.buffer.readMessage()); )
        this.onmessage?.(message);
    } catch {
      this.onerror?.(new Error("MCP input rejected"));
      void this.close();
    }
  };
  private readonly ended = () => {
    void this.close();
  };
  async start() {
    this.input.on("data", this.receive);
    this.input.on("end", this.ended);
    this.input.on("error", this.ended);
    this.output.on("error", this.ended);
  }
  async send(message: JSONRPCMessage) {
    const data = serializeMessage(message);
    if (
      this.closed ||
      Buffer.byteLength(data) > 262144 ||
      this.output.writableLength + Buffer.byteLength(data) > 1048576
    ) {
      await this.close();
      throw new Error("MCP backpressure");
    }
    await new Promise<void>((resolve, reject) =>
      this.output.write(data, (error) =>
        error ? reject(new Error("MCP output unavailable")) : resolve(),
      ),
    );
  }
  async close() {
    if (this.closed) return;
    this.closed = true;
    this.input
      .off("data", this.receive)
      .off("end", this.ended)
      .off("error", this.ended);
    this.output.off("error", this.ended);
    this.input.pause();
    this.buffer.clear();
    this.finish();
    this.onclose?.();
  }
}
const methods = new Set([
  "execution_software_observe",
  "execution_policy_tools",
  "execution_policy_status",
  "execution_tasks",
  "execution_capabilities",
  "execution_execute",
  "execution_status",
  "execution_cancel",
]);
/** MCP owns validation; this mapper supplies stage identity and reconciles against Rust. */
export async function connectExecution(
  input: Readable,
  output: Writable,
  resolveBinding: (request: {
    namespace: Namespace;
    commandId?: string;
    catalog?: Pick<
      import("@rss-mdm-agent/ai-contract/ports").ProviderConfiguration,
      "provider" | "config"
    >;
  }) => Promise<{
    binding: Pick<
      import("@rss-mdm-agent/ai-contract/wire").Binding,
      "provider" | "config"
    >;
    userGeneration: string;
    contextBinding: string;
  }>,
) {
  const client = new Client({ name: "rss-ai-host", version: "0.1.0" });
  const transport = new ParentTransport(input, output);
  try {
    await client.connect(transport, { timeout: 10000 });
    const catalog = await client.listTools({}, { timeout: 10000 });
    if (
      catalog.tools.length !== methods.size ||
      catalog.tools.some((t) => !methods.has(t.name))
    )
      throw new Error("execution tools mismatch");
  } catch {
    await client.close();
    await transport.close();
    throw new Error("execution connection unavailable");
  }
  const call = async (
    request: DeliveryRequest,
    name: string,
    args: Record<string, unknown>,
    b: Budget,
  ): Promise<any> => {
    const { binding, userGeneration, contextBinding } =
      await resolveBinding(request);
    if (
      !(["codex", "claude", "deepseek"] as const).includes(
        binding.provider as ExecutionOrigin["provider"],
      )
    )
      throw new Error("execution origin unavailable");
    const origin: ExecutionOrigin = {
      schemaVersion: 8,
      kind: "executionOrigin",
      namespace: request.namespace,
      userGeneration,
      contextBinding,
      operationId: request.body.operationId,
      provider: binding.provider as ExecutionOrigin["provider"],
      config: binding.config,
    };
    const reply = await client.callTool(
      {
        name,
        arguments: args,
        _meta: {
          "com.rss-mdm/ai-origin": origin,
        },
      },
      undefined,
      { signal: b.signal, timeout: b.timeoutMs, maxTotalTimeout: b.timeoutMs },
    );
    boundedJson(reply, defaultLimits);
    const value = reply.structuredContent as any;
    if (
      !value ||
      !["ok", "error"].includes(value.status) ||
      Boolean(reply.isError) !== (value.status === "error")
    )
      throw new Error("invalid execution reply");
    if (value.status === "error" && value.error?.code === "outcomeUnknown")
      throw new Error("execution outcome unknown");
    return value;
  };
  const receipt = (
    request: DeliveryRequest,
    value: unknown,
  ): DeliveryReceipt => ({
    receiptRef: request.body.operationId,
    reply: {
      disposition: (value as any).status === "error" ? "rejected" : "returned",
      text: boundedJson(value, defaultLimits),
    },
  });
  const router: DeliveryRouter = {
    catalog: async (namespace, b, catalogBinding) => {
      let stage = "binding";
      try {
        const { binding, userGeneration, contextBinding } =
          await resolveBinding({
            namespace,
            catalog: catalogBinding,
          });
        const origin: ExecutionOrigin = {
          schemaVersion: 8,
          kind: "executionOrigin",
          namespace,
          userGeneration,
          contextBinding,
          operationId: "directory",
          provider: binding.provider as ExecutionOrigin["provider"],
          config: binding.config,
        };
        stage = "policy-tools";
        const value = await client.callTool(
          {
            name: "execution_policy_tools",
            arguments: {},
            _meta: { "com.rss-mdm/ai-origin": origin },
          },
          undefined,
          { signal: b.signal, timeout: b.timeoutMs },
        );
        stage = "projection";
        boundedJson(value, defaultLimits);
        const body = value.structuredContent as any;
        if (body?.status !== "ok" || !Array.isArray(body.result)) {
          console.error("RSS_POLICY_DISCOVERY_UNAVAILABLE stage=projection");
          return fail("unavailable");
        }
        stage = "catalog";
        const catalog = await client.listTools(
          {},
          { signal: b.signal, timeout: b.timeoutMs },
        );
        return ok([
          ...catalog.tools.filter((t) => t.name !== "execution_policy_tools"),
          ...body.result,
        ]);
      } catch {
        console.error(`RSS_POLICY_DISCOVERY_UNAVAILABLE stage=${stage}`);
        return fail("unavailable");
      }
    },
    prepare: (namespace, proposal, commandId) => {
      if (
        !methods.has(proposal.name) &&
        !/^policy_[a-f0-9]{32}_[a-f0-9]{16}$/.test(proposal.name)
      )
        return fail("unsupported_capability");
      const read = [
        "execution_software_observe",
        "execution_policy_tools",
        "execution_policy_status",
        "execution_tasks",
        "execution_capabilities",
        "execution_status",
      ].includes(proposal.name);
      const id = proposal.name.startsWith("policy_")
        ? proposal.callId
        : read
          ? randomUUID()
          : operationRequestId(proposal.name, proposal.arguments);
      if (!isId(id)) return fail("invalid_input");
      const digest = createHash("sha256")
        .update(JSON.stringify([namespace, commandId, proposal.name, id]))
        .digest("hex");
      const operationId =
        proposal.name.startsWith("policy_") ||
        proposal.name === "execution_software_observe"
          ? `${digest.slice(0, 8)}-${digest.slice(8, 12)}-4${digest.slice(13, 16)}-a${digest.slice(17, 20)}-${digest.slice(20, 32)}`
          : digest;
      return ok({
        operationId,
        target: "rust-execution",
        permission: proposal.name === "execution_cancel" ? "ask" : "none",
      });
    },
    send: async (request, b) => {
      try {
        return ok(
          receipt(
            request,
            await call(
              request,
              request.body.proposal.name,
              request.body.proposal.arguments,
              b,
            ),
          ),
        );
      } catch {
        return fail("unavailable", "reconcile_first");
      }
    },
    reconcile: async (request, b) => {
      const { name, arguments: args } = request.body.proposal;
      try {
        if (name.startsWith("policy_")) {
          const status = await call(
            request,
            "execution_policy_status",
            { operationRequestId: request.body.operationId },
            b,
          );
          if (status.status === "error")
            return ok({
              state:
                status.error?.code === "notFound" ? "not_submitted" : "unknown",
            });
          return ok({ state: "committed", receipt: receipt(request, status) });
        }
        // Reads and exact immutable registrations are receiver-idempotent.
        if (!["execution_execute", "execution_cancel"].includes(name))
          return ok({
            state: "committed",
            receipt: receipt(request, await call(request, name, args, b)),
          });
        const status = await call(
          request,
          "execution_status",
          {
            operationRequestId: operationRequestId(
              request.body.proposal.name,
              request.body.proposal.arguments,
            ),
          },
          b,
        );
        if (status.status === "error")
          return ok({
            state:
              status.error?.code === "notFound" ? "not_submitted" : "unknown",
          });
        if (
          name === "execution_cancel" &&
          status.result.cancelRequested !== true &&
          !["cancelled", "verified", "failed"].includes(status.result.phase)
        )
          return ok({ state: "unknown" });
        return ok({ state: "committed", receipt: receipt(request, status) });
      } catch {
        return ok({ state: "unknown" });
      }
    },
    // MCP status reads do not consume the Rust result journal. Rust retains all business facts.
    acknowledge: async () => ok(undefined),
  };
  return { router, stopped: transport.stopped, close: () => client.close() };
}
