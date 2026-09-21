import { randomUUID } from "node:crypto";
import type { Duplex } from "node:stream";
import { boundedJson, type Budget } from "@rss-mdm-agent/ai-contract";

const peerCodes = [
  "authentication_required",
  "invalid_input",
  "context_unavailable",
  "unsupported_capability",
] as const;
type PeerCode = (typeof peerCodes)[number];
function peerCode(value: unknown): PeerCode | undefined {
  return peerCodes.find((code) => code === value);
}
export class PeerFailure extends Error {
  constructor(readonly code: PeerCode) {
    super(code);
  }
}
const maxFrame = 256 * 1024;
const limits = {
  maxBytes: maxFrame,
  maxTextBytes: 128 * 1024,
  maxDepth: 32,
  maxNodes: 16384,
};
export type Handler = (
  method: string,
  data: unknown,
  budget: Budget,
) => Promise<unknown>;
/** Private framed pipe. Size is checked before allocation or JSON parsing. */
export class Channel {
  private buffer = Buffer.alloc(0);
  private readonly pending = new Map<
    string,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
      cleanup: () => void;
    }
  >();
  private readonly active = new Map<string, AbortController>();
  private stopped = false;
  onEvent?: (data: unknown) => void;
  onClose?: () => void;
  constructor(
    private readonly pipe: Duplex,
    private readonly launchId: string,
    private readonly handler?: Handler,
  ) {
    pipe.on("data", (chunk: Buffer) => {
      try {
        if (chunk.length + this.buffer.length > maxFrame * 2)
          throw new Error("IPC buffer limit");
        this.buffer = Buffer.concat([this.buffer, chunk]);
        while (this.buffer.length >= 4) {
          const size = this.buffer.readUInt32BE(0);
          if (!size || size > maxFrame) throw new Error("IPC frame limit");
          if (this.buffer.length < size + 4) break;
          const packet = JSON.parse(
            this.buffer.subarray(4, size + 4).toString("utf8"),
          );
          this.buffer = this.buffer.subarray(size + 4);
          boundedJson(packet, limits);
          this.receive(packet);
        }
      } catch {
        this.close();
      }
    });
    pipe.on("error", () => this.close());
    pipe.on("end", () => this.close());
    pipe.on("close", () => this.close());
  }
  get closed() {
    return this.stopped;
  }
  private send(packet: Record<string, unknown>) {
    if (this.stopped) throw new Error("IPC closed");
    const bytes = Buffer.from(
      boundedJson({ ...packet, launchId: this.launchId }, limits),
    );
    if (this.pipe.writableLength + bytes.length > 1024 * 1024) {
      this.close();
      throw new Error("IPC backpressure");
    }
    const header = Buffer.alloc(4);
    header.writeUInt32BE(bytes.length);
    this.pipe.write(Buffer.concat([header, bytes]));
  }
  event(data: unknown) {
    this.send({ type: "event", data });
  }
  call(method: string, data: unknown, budget: Budget): Promise<unknown> {
    if (
      this.stopped ||
      budget.signal.aborted ||
      !Number.isSafeInteger(budget.timeoutMs) ||
      budget.timeoutMs <= 0 ||
      this.pending.size >= 64
    )
      return Promise.reject(new Error("IPC unavailable"));
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const abort = () => {
        this.pending.delete(id);
        cleanup();
        reject(new Error("IPC deadline"));
        try {
          this.send({ type: "cancel", id });
        } catch {}
      };
      const timer = setTimeout(abort, budget.timeoutMs);
      const cleanup = () => {
        clearTimeout(timer);
        budget.signal.removeEventListener("abort", abort);
      };
      budget.signal.addEventListener("abort", abort, { once: true });
      this.pending.set(id, { resolve, reject, cleanup });
      try {
        this.send({
          type: "call",
          id,
          method,
          data,
          timeoutMs: budget.timeoutMs,
        });
      } catch (error) {
        this.pending.delete(id);
        cleanup();
        reject(error);
      }
    });
  }
  private receive(packet: Record<string, unknown>) {
    if (!packet || packet.launchId !== this.launchId)
      throw new Error("IPC launch mismatch");
    if (packet.type === "event") {
      this.onEvent?.(packet.data);
      return;
    }
    if (typeof packet.id !== "string" || packet.id.length > 128)
      throw new Error("IPC id");
    const id = packet.id;
    if (packet.type === "reply" || packet.type === "failure") {
      const pending = this.pending.get(id);
      if (!pending) return;
      const code =
        packet.type === "failure" ? peerCode(packet.code) : undefined;
      if (packet.type === "failure" && packet.code !== undefined && !code)
        throw new Error("IPC error code");
      this.pending.delete(id);
      pending.cleanup();
      if (packet.type === "reply") pending.resolve(packet.data);
      else {
        pending.reject(
          code ? new PeerFailure(code) : new Error("IPC peer failure"),
        );
      }
    } else if (packet.type === "cancel") this.active.get(id)?.abort();
    else if (packet.type === "call") {
      if (
        !this.handler ||
        this.active.has(id) ||
        this.active.size >= 64 ||
        typeof packet.method !== "string" ||
        !Number.isSafeInteger(packet.timeoutMs) ||
        Number(packet.timeoutMs) <= 0 ||
        Number(packet.timeoutMs) > 2147483647
      )
        throw new Error("IPC request");
      const controller = new AbortController();
      this.active.set(id, controller);
      const timer = setTimeout(
        () => controller.abort(),
        Number(packet.timeoutMs),
      );
      void this.handler(packet.method, packet.data, {
        timeoutMs: Number(packet.timeoutMs),
        signal: controller.signal,
      })
        .then(
          (data) => {
            if (!controller.signal.aborted)
              this.send({ type: "reply", id, data: data ?? null });
          },
          (error: unknown) => {
            const raw =
              error && typeof error === "object" && "code" in error
                ? error.code
                : undefined;
            const code = peerCode(
              raw === "configuration_invalid" || raw === "configuration_file"
                ? "invalid_input"
                : raw,
            );
            if (!controller.signal.aborted)
              this.send({ type: "failure", id, ...(code ? { code } : {}) });
          },
        )
        .catch(() => this.close())
        .finally(() => {
          clearTimeout(timer);
          this.active.delete(id);
        });
    } else throw new Error("IPC packet");
  }
  close() {
    if (this.stopped) return;
    this.stopped = true;
    for (const p of this.pending.values()) {
      p.cleanup();
      p.reject(new Error("IPC closed"));
    }
    this.pending.clear();
    for (const a of this.active.values()) a.abort();
    this.active.clear();
    this.buffer = Buffer.alloc(0);
    this.pipe.destroy();
    this.onClose?.();
  }
}
