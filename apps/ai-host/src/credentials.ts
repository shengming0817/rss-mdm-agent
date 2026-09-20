import { createConnection } from "node:net";
import { ConfigurationError } from "./configuration.js";
/** Private native socket; secrets never pass through ACP, the UI, or a configuration file. */
export async function nativeCredential<T>(
  socketPath: string,
  request: object,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    let buffer = Buffer.alloc(0),
      finished = false;
    const fail = () => {
      if (finished) return;
      finished = true;
      socket.destroy();
      reject(new ConfigurationError("authentication_required"));
    };
    socket.setTimeout(15000, fail);
    socket.on("error", fail);
    socket.on("end", fail);
    socket.once("connect", () => socket.write(JSON.stringify(request) + "\n"));
    socket.on("data", (chunk: Buffer) => {
      buffer = Buffer.concat([buffer, chunk]);
      if (buffer.length > 262144) return fail();
      const end = buffer.indexOf(10);
      if (end < 0) return;
      try {
        const value = JSON.parse(buffer.subarray(0, end).toString("utf8"));
        if (value.ok !== true) return fail();
        finished = true;
        socket.destroy();
        resolve(value.value as T);
      } catch {
        fail();
      }
    });
  });
}
