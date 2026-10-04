import { createServer, type Server as HttpServer } from "node:http";
import { randomUUID, timingSafeEqual } from "node:crypto";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import {
  ListToolsRequestSchema,
  CallToolRequestSchema,
  ListResourcesRequestSchema,
  ListResourceTemplatesRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import {
  boundedJson,
  isId,
  withinBudget,
  type Budget,
  type ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
import { bounded, limits } from "./support.js";

export const HOST_SERVER = "rss_host";
function toolResult(
  disposition: "returned" | "rejected" | "unavailable",
  text: string,
) {
  return {
    isError: disposition !== "returned",
    content: [{ type: "text" as const, text }],
    ...(() => {
      try {
        return { structuredContent: JSON.parse(text) };
      } catch {
        return {};
      }
    })(),
    _meta: { "com.rss-mdm/host-disposition": disposition },
  };
}
/** Incarnation-local endpoint. Native tools never receive an execution permit. */
export class ToolBridge {
  private server: HttpServer;
  private connections = new Set<Server>();
  private abort = new AbortController();
  readonly token = randomUUID();
  catalog: readonly import("@modelcontextprotocol/sdk/types.js").Tool[] = [];
  url?: string;
  constructor(
    private readonly endpoint: ToolEndpoint,
    private readonly canPropose: () => boolean,
    private readonly observe?: (proposal: {
      name: string;
      arguments: Record<string, unknown>;
      callId: string;
    }) => (result: {
      disposition: "returned" | "rejected" | "unavailable";
      text: string;
    }) => void,
  ) {
    this.server = createServer(async (request, response) => {
      const actual = Buffer.from(request.headers.authorization ?? ""),
        expected = Buffer.from(`Bearer ${this.token}`);
      if (
        request.method !== "POST" ||
        request.url !== "/mcp" ||
        actual.length !== expected.length ||
        !timingSafeEqual(actual, expected) ||
        request.headers.origin !== undefined ||
        this.abort.signal.aborted
      ) {
        response.writeHead(403).end();
        return;
      }
      const mcp = new Server(
        { name: HOST_SERVER, version: "0.1.0" },
        { capabilities: { tools: {}, resources: {} } },
      );
      const transport = new StreamableHTTPServerTransport({
        sessionIdGenerator: undefined,
        enableJsonResponse: true,
      });
      this.connections.add(mcp);
      mcp.setRequestHandler(ListToolsRequestSchema, async () => ({
        tools: this.catalog,
      }));
      mcp.setRequestHandler(ListResourcesRequestSchema, async () => ({
        resources: [],
      }));
      mcp.setRequestHandler(ListResourceTemplatesRequestSchema, async () => ({
        resourceTemplates: [],
      }));
      mcp.setRequestHandler(ReadResourceRequestSchema, async () => {
        throw new Error("Resource access is unsupported");
      });
      mcp.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
        const input = params.arguments;
        if (
          !this.catalog.some((tool) => tool.name === params.name) ||
          !input ||
          Array.isArray(input) ||
          !this.canPropose()
        )
          return toolResult("unavailable", "Tool unavailable");
        const proposal = {
          name: params.name,
          arguments: input,
          callId: randomUUID(),
        };
        const observed = this.observe?.(proposal);
        try {
          boundedJson(input, limits);
          const result = await withinBudget(
            () => ({ timeoutMs: 30000, signal: this.abort.signal }),
            (budget) => this.endpoint.propose(proposal, budget),
          );
          if (!result.ok || this.abort.signal.aborted)
            throw new Error("unavailable");
          boundedJson(result.value, limits);
          if (
            !["returned", "rejected", "unavailable"].includes(
              result.value.disposition,
            ) ||
            typeof result.value.text !== "string" ||
            result.value.text.length > 65536
          )
            throw new Error("invalid host result");
          const response = toolResult(
            result.value.disposition,
            result.value.text,
          );
          if (
            result.value.disposition === "returned" &&
            this.catalog.find((t) => t.name === params.name)?.outputSchema &&
            !("structuredContent" in response)
          )
            throw new Error("Missing structured tool output");
          observed?.(result.value);
          return response;
        } catch {
          observed?.({
            disposition: "unavailable",
            text: "Host proposal unavailable",
          });
          return toolResult("unavailable", "Host proposal unavailable");
        }
      });
      response.once("close", () => {
        this.connections.delete(mcp);
        void mcp.close();
      });
      try {
        await mcp.connect(transport);
        await transport.handleRequest(request, response);
      } catch {
        if (!response.headersSent) response.writeHead(500);
        response.end();
        this.connections.delete(mcp);
        await mcp.close();
      }
    });
    this.server.maxConnections = 16;
    this.server.requestTimeout = 35000;
    this.server.headersTimeout = 5000;
  }
  async start(budget: Budget): Promise<void> {
    const listed = await bounded(this.endpoint.list(budget), budget);
    if (
      !listed.ok ||
      listed.value.length > 128 ||
      listed.value.some((t) => !isId(t.name))
    )
      throw new Error("Tool discovery unavailable");
    boundedJson(listed.value, limits);
    this.catalog = structuredClone(listed.value);
    await bounded(
      new Promise<void>((resolve, reject) => {
        this.server.once("error", reject);
        this.server.listen(0, "127.0.0.1", () => {
          if (this.abort.signal.aborted) {
            this.server.close();
            reject(new Error("closed bridge"));
            return;
          }
          const address = this.server.address();
          if (!address || typeof address === "string")
            return reject(new Error("invalid listener"));
          this.url = `http://127.0.0.1:${address.port}/mcp`;
          resolve();
        });
      }),
      budget,
    );
  }
  quiesce(): void {
    this.abort.abort();
  }
  async close(budget: Budget): Promise<void> {
    this.abort.abort();
    const close = new Promise<void>((resolve) =>
      this.server.close(() => resolve()),
    );
    this.server.closeAllConnections();
    await bounded(
      Promise.all([close, ...[...this.connections].map((mcp) => mcp.close())]),
      budget,
    );
  }
}
