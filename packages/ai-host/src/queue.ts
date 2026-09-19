/** Bounded transient output. Durable work lives exclusively in SessionStore. */
export class Output<T> implements AsyncIterable<T> {
  private values: { value: T; bytes: number }[] = [];
  private bytes = 0;
  private ended = false;
  private wake?: () => void;
  constructor(
    private readonly maxItems = 256,
    private readonly maxBytes = 1024 * 1024,
  ) {}
  push(value: T): boolean {
    if (this.ended) return false;
    const bytes = Buffer.byteLength(JSON.stringify(value));
    if (
      this.values.length >= this.maxItems ||
      this.bytes + bytes > this.maxBytes
    )
      return false;
    this.values.push({ value, bytes });
    this.bytes += bytes;
    this.wake?.();
    return true;
  }
  finish() {
    this.ended = true;
    this.wake?.();
  }
  end(last?: T) {
    this.values = [];
    this.bytes = 0;
    if (last !== undefined) this.values.push({ value: last, bytes: 0 });
    this.ended = true;
    this.wake?.();
  }
  async *[Symbol.asyncIterator]() {
    for (;;) {
      const item = this.values.shift();
      if (item) {
        this.bytes -= item.bytes;
        yield item.value;
        continue;
      }
      if (this.ended) return;
      await new Promise<void>((resolve) => {
        this.wake = resolve;
      });
      this.wake = undefined;
    }
  }
}
