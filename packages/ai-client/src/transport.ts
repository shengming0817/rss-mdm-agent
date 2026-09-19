import { ClientError, clientError } from "./errors.js";
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
      try {
        dispose();
      } catch {
        throw new ClientError("transport_failed");
      } finally {
        endRead();
      }
    }
  };
  const readable = new ReadableStream<AnyMessage>(
    {
      start(controller) {
        endRead = () => controller.close();
        try {
          dispose = channel.listen((message) => {
            if (closed) return;
            try {
              if ((controller.desiredSize ?? 0) <= 0)
                throw new ClientError("transport_capacity");
              controller.enqueue(JSON.parse(boundedJson(message, limits)));
            } catch (error) {
              endRead = () => {};
              controller.error(clientError(error, "invalid_transport_message"));
              close();
            }
          }, close);
          if (closed) dispose();
        } catch {
          endRead = () => {};
          controller.error(new ClientError("transport_failed"));
          close();
        }
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
      if (closed) throw new ClientError("transport_closed");
      try {
        await channel.send(JSON.parse(boundedJson(message, limits)));
      } catch (error) {
        close();
        throw clientError(error, "transport_failed");
      }
    },
    close,
    abort: close,
  });
  return { readable, writable };
}
export { ndJsonStream } from "@agentclientprotocol/sdk";
export type { Stream } from "@agentclientprotocol/sdk";
