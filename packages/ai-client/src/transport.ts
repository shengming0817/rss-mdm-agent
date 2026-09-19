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
  let endRead = () => {};
  let closed = false;
  const close = () => {
    if (!closed) {
      closed = true;
      dispose();
      endRead();
    }
  };
  const readable = new ReadableStream<AnyMessage>(
    {
      start(controller) {
        endRead = () => controller.close();
        dispose = channel.listen((message) => {
          if (closed) return;
          try {
            if ((controller.desiredSize ?? 0) <= 0)
              throw new Error("transport capacity");
            controller.enqueue(JSON.parse(boundedJson(message, limits)));
          } catch {
            endRead = () => {};
            controller.error(new Error("invalid transport message"));
            close();
          }
        }, close);
        if (closed) dispose();
      },
      cancel: () => {
        endRead = () => {};
        close();
      },
    },
    { highWaterMark: 64 },
  );
  const writable = new WritableStream<AnyMessage>({
    write: async (message) => {
      if (closed) throw new Error("transport closed");
      try {
        await channel.send(JSON.parse(boundedJson(message, limits)));
      } catch (error) {
        close();
        throw error;
      }
    },
    close,
    abort: close,
  });
  return { readable, writable };
}
export { ndJsonStream } from "@agentclientprotocol/sdk";
export type { Stream } from "@agentclientprotocol/sdk";
