import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import {
  ReadBuffer,
  serializeMessage,
} from "@modelcontextprotocol/sdk/shared/stdio.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import type { JSONRPCMessage } from "@modelcontextprotocol/sdk/types.js";
import type { Readable, Writable } from "node:stream";
import { createHash, randomUUID } from "node:crypto";
import {
  boundedJson,
  isId,
  type Budget,
  type ExecutionOrigin,
} from "@rss-mdm-agent/ai-contract";
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
  "execution_catalog",
  "execution_capabilities",
  "execution_preview",
  "execution_propose",
  "execution_submit",
  "execution_status",
  "execution_cancel",
]);
function businessId(proposal: DeliveryRequest["body"]["proposal"]): unknown {
  const args = proposal.arguments;
  const variant = args.catalog ?? args.candidate ?? args.script ?? args;
  return typeof variant === "object" && variant !== null
    ? ((variant as any).selection?.operationRequestId ??
        (variant as any).operationRequestId)
    : undefined;
}
/** MCP owns validation; this mapper supplies stage identity and reconciles against Rust. */
export async function connectExecution(
  input: Readable,
  output: Writable,
  resolveBinding: (request: DeliveryRequest) => Promise<{
    binding: import("@rss-mdm-agent/ai-contract").Binding;
    userGeneration: string;
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
    const { binding, userGeneration } = await resolveBinding(request);
    if (
      !(["codex", "claude", "deepseek"] as const).includes(
        binding.provider as ExecutionOrigin["provider"],
      )
    )
      throw new Error("execution origin unavailable");
    const origin: ExecutionOrigin = {
      schemaVersion: 5,
      kind: "executionOrigin",
      namespace: request.namespace,
      userGeneration,
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
    prepare: (namespace, proposal) => {
      if (!methods.has(proposal.name)) return fail("unsupported_capability");
      const read = [
        "execution_catalog",
        "execution_capabilities",
        "execution_status",
      ].includes(proposal.name);
      const id = read ? randomUUID() : businessId(proposal);
      if (!isId(id)) return fail("invalid_input");
      const operationId = createHash("sha256")
        .update(JSON.stringify([namespace, proposal.name, id]))
        .digest("hex");
      return ok({ operationId, target: "rust-execution" });
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
        // These reads and exact immutable registrations are receiver-idempotent and never dispatch.
        if (!["execution_submit", "execution_cancel"].includes(name))
          return ok({
            state: "committed",
            receipt: receipt(request, await call(request, name, args, b)),
          });
        const status = await call(
          request,
          "execution_status",
          { operationRequestId: businessId(request.body.proposal) },
          b,
        );
        if (status.status === "error")
          return ok({
            state:
              status.error?.code === "notFound" ? "not_submitted" : "unknown",
          });
        if (name === "execution_submit") {
          const plan = args.plan as Record<string, unknown>;
          if (
            status.result.plan?.planId !== plan?.planId ||
            status.result.plan?.digest !== plan?.digest
          )
            return ok({
              state: "committed",
              receipt: receipt(request, {
                status: "error",
                error: { code: "conflict" },
              }),
            });
          if (status.result.submitted !== true)
            return ok({
              state:
                status.result.submitted === false ? "not_submitted" : "unknown",
            });
        }
        if (
          name === "execution_cancel" &&
          status.result.cancelRequested !== true &&
          !["cancelled", "testCompleted", "failed"].includes(
            status.result.phase,
          )
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
