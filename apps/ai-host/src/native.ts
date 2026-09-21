import { Socket } from "node:net";
import type { Duplex } from "node:stream";
import { StringDecoder } from "node:string_decoder";
import {
  decode,
  type NativeCall,
  type NativeControlFrame,
  type NativeEvent,
} from "@rss-mdm-agent/ai-contract";
import { defaultLimits } from "@rss-mdm-agent/ai-contract/transitions";
type Method = NativeCall["method"];
type Call<M extends Method> = Extract<NativeCall, { method: M }>;
const nativeLimits = {
  ...defaultLimits,
  maxBytes: 524288,
  maxTextBytes: 524288,
  maxDepth: 64,
};
/** Private inherited parent descriptor. Request IDs correlate replies; they grant no authority. */
export class NativeControl {
  private sequence = 0;
  private pending = new Map<
    number,
    {
      resolve(value: unknown): void;
      reject(error: Error): void;
      timer: NodeJS.Timeout;
    }
  >();
  private ended = false;
  private inFlight = 0;
  readonly stopped: Promise<void>;
  private finish!: () => void;
  constructor(
    private readonly request: (call: NativeCall) => Promise<unknown>,
    private readonly event: (event: NativeEvent) => void,
    private readonly socket: Duplex = new Socket({
      fd: 3,
      readable: true,
      writable: true,
    }),
  ) {
    this.stopped = new Promise((resolve) => {
      this.finish = resolve;
    });
    const decoder = new StringDecoder("utf8");
    let buffer = "";
    socket.on("data", (chunk) => {
      try {
        buffer += decoder.write(chunk);
        let end: number;
        while ((end = buffer.indexOf("\n")) >= 0) {
          const line = buffer.slice(0, end);
          buffer = buffer.slice(end + 1);
          if (Buffer.byteLength(line) > 524288) throw new Error();
          const frame = decode(line, nativeLimits);
          if (
            frame.kind !== "nativeCall" &&
            frame.kind !== "nativeReply" &&
            frame.kind !== "nativeEvent"
          )
            throw new Error();
          this.receive(frame);
        }
        if (Buffer.byteLength(buffer) > 524288) throw new Error();
      } catch {
        this.close();
      }
    });
    socket
      .on("error", () => this.close())
      .on("end", () => this.close())
      .on("close", () => this.close());
  }
  private receive(frame: NativeControlFrame) {
    if (frame.kind === "nativeReply") {
      const pending = this.pending.get(frame.id);
      if (!pending) return;
      this.pending.delete(frame.id);
      clearTimeout(pending.timer);
      frame.ok === true
        ? pending.resolve(frame.value)
        : pending.reject(new Error("native unavailable"));
    } else if (frame.kind === "nativeEvent") {
      this.event(frame);
    } else if (frame.kind === "nativeCall" && this.inFlight < 16) {
      this.inFlight++;
      void this.request(frame)
        .then(
          (value) =>
            this.send({
              schemaVersion: 5,
              kind: "nativeReply",
              id: frame.id,
              ok: true,
              value,
            }),
          () =>
            this.send({
              schemaVersion: 5,
              kind: "nativeReply",
              id: frame.id,
              ok: false,
            }),
        )
        .catch(() => this.close())
        .finally(() => {
          this.inFlight--;
        });
    } else throw new Error("invalid native frame");
  }
  private send(frame: NativeControlFrame) {
    const line = JSON.stringify(frame) + "\n";
    if (
      this.ended ||
      Buffer.byteLength(line) > 524288 ||
      this.socket.writableLength > 1048576
    )
      throw new Error("native unavailable");
    this.socket.write(line);
  }
  emit(channel: string, message: unknown) {
    this.send({ schemaVersion: 5, kind: "nativeEvent", channel, message });
  }
  call<M extends Method>(method: M, data: Call<M>["data"]): Promise<unknown> {
    if (this.ended || this.pending.size >= 16)
      return Promise.reject(new Error("native unavailable"));
    return new Promise((resolve, reject) => {
      const id = ++this.sequence;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("native timeout"));
      }, 10000);
      this.pending.set(id, { resolve, reject, timer });
      try {
        this.send({
          schemaVersion: 5,
          kind: "nativeCall",
          id,
          method,
          data,
        } as Call<M>);
      } catch (error) {
        this.pending.delete(id);
        clearTimeout(timer);
        reject(error);
      }
    });
  }
  close() {
    if (this.ended) return;
    this.ended = true;
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error("native unavailable"));
    }
    this.pending.clear();
    this.socket.destroy();
    this.finish();
  }
}
