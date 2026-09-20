import { Socket } from "node:net";
import type { Duplex } from "node:stream";
import { StringDecoder } from "node:string_decoder";
/** Private inherited parent descriptor. Request IDs correlate replies; they grant no authority. */
export class NativeControl {
  private sequence = 0;
  private pending = new Map<
    number,
    {
      resolve(value: any): void;
      reject(error: Error): void;
      timer: NodeJS.Timeout;
    }
  >();
  private ended = false;
  private inFlight = 0;
  readonly stopped: Promise<void>;
  private finish!: () => void;
  constructor(
    private readonly request: (method: string, data: any) => Promise<unknown>,
    private readonly event: (channel: string, message: any) => void,
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
          this.receive(JSON.parse(line));
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
  private receive(frame: any) {
    if (frame.type === "reply") {
      const pending = this.pending.get(frame.id);
      if (!pending) return;
      this.pending.delete(frame.id);
      clearTimeout(pending.timer);
      frame.ok === true
        ? pending.resolve(frame.value)
        : pending.reject(new Error("native unavailable"));
    } else if (frame.type === "event" && typeof frame.channel === "string") {
      this.event(frame.channel, frame.message);
    } else if (
      frame.type === "call" &&
      Number.isSafeInteger(frame.id) &&
      typeof frame.method === "string" &&
      this.inFlight < 16
    ) {
      this.inFlight++;
      void this.request(frame.method, frame.data)
        .then(
          (value) =>
            this.send({ type: "reply", id: frame.id, ok: true, value }),
          () => this.send({ type: "reply", id: frame.id, ok: false }),
        )
        .catch(() => this.close())
        .finally(() => {
          this.inFlight--;
        });
    } else throw new Error("invalid native frame");
  }
  private send(frame: unknown) {
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
    this.send({ type: "event", channel, message });
  }
  call(method: string, data: unknown): Promise<any> {
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
        this.send({ type: "call", id, method, data });
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
