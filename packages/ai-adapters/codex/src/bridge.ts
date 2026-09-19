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
  type Budget,
  type ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
import { bounded, limits } from "./support.js";

export const HOST_SERVER = "rss_host";
export const HOST_TOOL = {
  name: "propose",
  description:
    "Submit a business tool proposal to the authenticated host; this does not grant execution authority.",
  inputSchema: {
    type: "object" as const,
    properties: {
      name: { type: "string" },
      arguments: { type: "object", additionalProperties: true },
    },
    required: ["name", "arguments"],
    additionalProperties: false,
  },
};
function toolResult(
  disposition: "returned" | "rejected" | "unavailable",
  text: string,
) {
  return {
    isError: disposition !== "returned",
    content: [{ type: "text" as const, text }],
    structuredContent: { rssHostResult: { disposition, text } },
  };
}
/** Incarnation-local endpoint. Native tools never receive an execution permit. */
export class ToolBridge {
  private server: HttpServer;
  private connections = new Set<Server>();
  private abort = new AbortController();
  readonly token = randomUUID();
  url?: string;
  constructor(
    private readonly endpoint: ToolEndpoint,
    private readonly canPropose: () => boolean,
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
        tools: [HOST_TOOL],
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
          params.name !== HOST_TOOL.name ||
          !input ||
          Object.keys(input).sort().join(",") !== "arguments,name" ||
          !isId(input.name) ||
          !input.arguments ||
          typeof input.arguments !== "object" ||
          Array.isArray(input.arguments) ||
          !this.canPropose()
        )
          return toolResult("unavailable", "Host proposal unavailable");
        try {
          boundedJson(input, limits);
          // ref: node:globals AbortSignal.any/timeout; the Host receives the same deadline as the waiter.
          const budget = {
            timeoutMs: 30000,
            signal: AbortSignal.any([
              this.abort.signal,
              AbortSignal.timeout(30000),
            ]),
          };
          const result = await bounded(
            this.endpoint.propose(
              {
                name: input.name as string,
                arguments: input.arguments as Record<string, unknown>,
              },
              budget,
            ),
            budget,
          );
          if (!result.ok || budget.signal.aborted)
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
          return toolResult(result.value.disposition, result.value.text);
        } catch {
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
