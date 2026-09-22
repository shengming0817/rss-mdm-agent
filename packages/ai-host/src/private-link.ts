import { Duplex, type Readable, type Writable } from "node:stream";

const laneSets = {
  native: ["native", "execution"],
  worker: ["control", "tools", "events"],
} as const;
type Role = keyof typeof laneSets;
const maxFrame = 512 * 1024;
const maxQueued = 1024 * 1024;
const magic = Buffer.from([82, 83, 83, 1]);

/** Fixed private-process transport V1. No discovery, listener or legacy mode.
 * A link has one reader/writer; every lane has independent bounded capacity.
 * Native/control traffic is drained first. A stuck physical pipe is bounded by
 * its owner's OS-level shutdown deadline, not by an in-band close message.
 */
export class PrivateLink {
  private buffer = Buffer.alloc(0);
  private ended = false;
  private draining = false;
  private waiting = false;
  private readonly lanes: Duplex[];
  private readonly queues: Buffer[][];
  private readonly queued: number[];
  private readonly names: readonly string[];
  onClose?: () => void;
  constructor(
    private readonly input: Readable,
    private readonly output: Writable,
    role: Role,
  ) {
    this.names = laneSets[role];
    this.queues = this.names.map(() => []);
    this.queued = this.names.map(() => 0);
    this.lanes = this.names.map((_, id) => {
      const lane = new Duplex({
        readableHighWaterMark: maxQueued,
        writableHighWaterMark: maxQueued,
        read() {},
        write: (data: Buffer, _encoding, done) => {
          if (
            this.ended ||
            !data.length ||
            data.length > maxFrame ||
            this.queued[id] + data.length > maxQueued ||
            this.queues[id].length >= 64
          ) {
            this.close();
            done(new Error("private link unavailable"));
            return;
          }
          const frame = Buffer.allocUnsafe(9 + data.length);
          magic.copy(frame);
          frame[4] = id;
          frame.writeUInt32BE(data.length, 5);
          data.copy(frame, 9);
          this.queues[id].push(frame);
          this.queued[id] += data.length;
          // Defer one tick so control can preempt queued bulk frames.
          queueMicrotask(() => this.flush());
          done();
        },
        destroy: (_error, done) => {
          this.close();
          done();
        },
      });
      lane.on("error", () => this.close());
      return lane;
    });
    input.on("data", (data: Buffer) => {
      try {
        if (this.ended) return;
        if (this.buffer.length + data.length > maxFrame * 3) throw new Error();
        this.buffer = Buffer.concat([this.buffer, data]);
        while (this.buffer.length >= 9) {
          if (!this.buffer.subarray(0, 4).equals(magic)) throw new Error();
          const id = this.buffer[4],
            size = this.buffer.readUInt32BE(5);
          if (id >= this.lanes.length || !size || size > maxFrame)
            throw new Error();
          if (this.buffer.length < size + 9) break;
          const lane = this.lanes[id];
          if (lane.readableLength + size > maxQueued) throw new Error();
          lane.push(Buffer.from(this.buffer.subarray(9, size + 9)));
          this.buffer = this.buffer.subarray(size + 9);
        }
      } catch {
        this.close();
      }
    });
    input
      .on("end", () => this.close())
      .on("error", () => this.close())
      .on("close", () => this.close());
    output.on("error", () => this.close()).on("close", () => this.close());
    output.on("drain", () => {
      this.waiting = false;
      this.flush();
    });
  }
  private flush() {
    if (this.ended || this.draining || this.waiting) return;
    this.draining = true;
    try {
      for (;;) {
        const id = this.queues.findIndex((q) => q.length > 0);
        if (id < 0) break;
        const frame = this.queues[id].shift()!;
        this.queued[id] -= frame.length - 9;
        if (!this.output.write(frame)) {
          this.waiting = true;
          break;
        }
      }
    } catch {
      this.close();
    } finally {
      this.draining = false;
    }
  }
  lane(name: string): Duplex {
    const id = this.names.indexOf(name);
    if (id < 0) throw new Error("unknown private lane");
    return this.lanes[id];
  }
  get closed() {
    return this.ended;
  }
  close() {
    if (this.ended) return;
    this.ended = true;
    this.buffer = Buffer.alloc(0);
    for (const q of this.queues) q.length = 0;
    for (const lane of this.lanes) lane.destroy();
    this.input.destroy();
    this.output.destroy();
    this.onClose?.();
  }
}
