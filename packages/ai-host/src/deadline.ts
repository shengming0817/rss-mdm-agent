import type { Budget } from "@rss-mdm-agent/ai-contract";

/** One absolute deadline across all awaits; aborting a wait is not proof of completion. */
export class Deadline {
  private readonly abort = new AbortController();
  private readonly end: number;
  private readonly timer: ReturnType<typeof setTimeout>;
  private readonly cancel = () => this.abort.abort();
  constructor(private readonly input: Budget) {
    this.end = Date.now() + input.timeoutMs;
    this.timer = setTimeout(this.cancel, Math.max(1, input.timeoutMs));
    input.signal.addEventListener("abort", this.cancel, { once: true });
    if (input.signal.aborted) this.cancel();
  }
  budget(): Budget {
    return {
      timeoutMs: Math.max(1, this.end - Date.now()),
      signal: this.abort.signal,
    };
  }
  check(): void {
    if (this.abort.signal.aborted || Date.now() >= this.end) {
      this.cancel();
      throw new Error("lifecycle deadline");
    }
  }
  async wait<T>(action: () => Promise<T>): Promise<T> {
    this.check();
    const signal = this.abort.signal;
    let cancel: () => void = () => {};
    try {
      return await Promise.race([
        new Promise<never>((_, reject) => {
          cancel = () => reject(new Error("lifecycle deadline"));
          signal.addEventListener("abort", cancel, { once: true });
        }),
        Promise.resolve().then(() => {
          this.check();
          return action();
        }),
      ]);
    } finally {
      signal.removeEventListener("abort", cancel);
    }
  }
  dispose(): void {
    clearTimeout(this.timer);
    this.input.signal.removeEventListener("abort", this.cancel);
    this.cancel();
  }
}
