import {
  boundedJson,
  accessLimits,
  type Limits,
} from "@rss-mdm-agent/ai-contract";
import type { AnyMessage, Stream } from "@agentclientprotocol/sdk";

/** Official SDK Stream seam; structured messages remain the original JSON-RPC envelope. */
export function localTransportPair(): [Stream, Stream] {
  const a = new TransformStream<AnyMessage, AnyMessage>(),
    b = new TransformStream<AnyMessage, AnyMessage>();
  return [
    { readable: a.readable, writable: b.writable },
    { readable: b.readable, writable: a.writable },
  ];
}
export interface MessageChannel {
  send(message: AnyMessage): Promise<void>;
  /** The composition root supplies delivery and disconnect notifications. */
  listen(
    receive: (message: unknown) => void,
    disconnect: () => void,
  ): () => void;
}
/** Adapt an injected Tauri/local channel without importing Tauri or implementing RPC.
 * Browser pages and desktop composition use the same SDK callback lifecycle. */
export function channelStream(
  channel: MessageChannel,
  limits: Limits = accessLimits,
): Stream {
  let dispose = () => {};
  let closed = false;
  const close = () => {
    if (!closed) {
      closed = true;
      dispose();
    }
  };
  const readable = new ReadableStream<AnyMessage>(
    {
      start(controller) {
        dispose = channel.listen(
          (message) => {
            if (closed) return;
            try {
              if ((controller.desiredSize ?? 0) <= 0)
                throw new Error("transport capacity");
              controller.enqueue(JSON.parse(boundedJson(message, limits)));
            } catch {
              controller.error(new Error("invalid transport message"));
              close();
            }
          },
          () => {
            if (!closed) {
              controller.close();
              close();
            }
          },
        );
        if (closed) dispose();
      },
      cancel: close,
    },
    { highWaterMark: 64 },
  );
  const writable = new WritableStream<AnyMessage>({
    write: (message) => {
      if (closed) throw new Error("transport closed");
      return channel.send(JSON.parse(boundedJson(message, limits)));
    },
    close,
    abort: close,
  });
  return { readable, writable };
}
export { ndJsonStream } from "@agentclientprotocol/sdk";
export type { Stream } from "@agentclientprotocol/sdk";
