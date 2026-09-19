import { boundedJson, type Limits } from "./codec.js";
import { accessLimits } from "./a2ui.js";

/** Budget the original SDK envelope at ingress and egress; no RPC interpretation. */
export function boundedStream<T>(
  stream: { readable: ReadableStream<T>; writable: WritableStream<T> },
  limits: Limits = accessLimits,
) {
  const checked = (message: T): T =>
    JSON.parse(boundedJson(message, limits)) as T;
  const readable = stream.readable.pipeThrough(
    new TransformStream<T, T>({
      transform(message, controller) {
        controller.enqueue(checked(message));
      },
    }),
  );
  const writer = stream.writable.getWriter();
  const writable = new WritableStream<T>({
    write: (message) => writer.write(checked(message)),
    close: () => writer.close().finally(() => writer.releaseLock()),
    abort: (reason) => writer.abort(reason).finally(() => writer.releaseLock()),
  });
  return { readable, writable };
}
