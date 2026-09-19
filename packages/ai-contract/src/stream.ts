import { boundedJson, type Limits } from "./codec.js";
import { accessLimits } from "./a2ui.js";

/** Budget the original SDK envelope; either half closing terminates its transport peer. */
export function boundedStream<T>(
  stream: { readable: ReadableStream<T>; writable: WritableStream<T> },
  limits: Limits = accessLimits,
) {
  const reader = stream.readable.getReader(),
    writer = stream.writable.getWriter();
  const checked = (message: T): T =>
    JSON.parse(boundedJson(message, limits)) as T;
  let stopped = false,
    controller: ReadableStreamDefaultController<T>;
  const stop = (reason: unknown = new Error("transport closed")) => {
    if (stopped) return;
    stopped = true;
    controller.error(reason);
    void Promise.allSettled([reader.cancel(reason), writer.abort(reason)]).then(
      () => {
        reader.releaseLock();
        writer.releaseLock();
      },
    );
  };
  const readable = new ReadableStream<T>({
    start(value) {
      controller = value;
    },
    async pull() {
      try {
        const item = await reader.read();
        if (stopped) return;
        if (item.done) {
          stop();
          return;
        }
        controller.enqueue(checked(item.value));
      } catch (error) {
        stop(error);
      }
    },
    cancel: stop,
  });
  const writable = new WritableStream<T>({
    async write(message) {
      if (stopped) throw new Error("transport closed");
      try {
        await writer.write(checked(message));
      } catch (error) {
        stop(error);
        throw error;
      }
    },
    close: () => stop(),
    abort: stop,
  });
  return { readable, writable };
}
