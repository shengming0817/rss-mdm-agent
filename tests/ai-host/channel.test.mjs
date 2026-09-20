import { test } from "node:test";
import assert from "node:assert/strict";
import { Duplex, PassThrough } from "node:stream";
import { Channel, PeerFailure } from "../../packages/ai-host/dist/channel.js";

test("worker failures preserve only closed recovery codes and never serialize provider errors", async () => {
  const up = new PassThrough(),
    down = new PassThrough(),
    wire = [];
  down.on("data", (bytes) => wire.push(bytes));
  const left = new Channel(
    Duplex.from({ readable: down, writable: up }),
    "launch",
  );
  const right = new Channel(
    Duplex.from({ readable: up, writable: down }),
    "launch",
    async (code) => {
      throw Object.assign(new Error("SECRET_CANARY_provider_path_and_key"), {
        code,
        details: "SECRET_CANARY",
      });
    },
  );
  try {
    for (const code of [
      "authentication_required",
      "invalid_input",
      "context_unavailable",
      "unsupported_capability",
    ]) {
      await assert.rejects(
        left.call(
          code,
          {},
          { timeoutMs: 1000, signal: new AbortController().signal },
        ),
        (error) =>
          error instanceof PeerFailure &&
          error.code === code &&
          error.message === code,
      );
    }
    await assert.rejects(
      left.call(
        "configuration_file",
        {},
        { timeoutMs: 1000, signal: new AbortController().signal },
      ),
      { code: "invalid_input" },
    );
    await assert.rejects(
      left.call(
        "SECRET_CANARY_unknown_code",
        {},
        { timeoutMs: 1000, signal: new AbortController().signal },
      ),
      (error) =>
        !(error instanceof PeerFailure) && error.message === "IPC peer failure",
    );
    assert.equal(Buffer.concat(wire).includes("SECRET_CANARY"), false);
  } finally {
    left.close();
    right.close();
  }
});

test("an unknown wire failure code closes and rejects its pending request", async () => {
  const up = new PassThrough(),
    down = new PassThrough();
  const left = new Channel(
    Duplex.from({ readable: down, writable: up }),
    "launch",
  );
  up.once("data", (bytes) => {
    const request = JSON.parse(bytes.subarray(4).toString());
    const payload = Buffer.from(
      JSON.stringify({
        launchId: "launch",
        type: "failure",
        id: request.id,
        code: "arbitrary",
      }),
    );
    const header = Buffer.alloc(4);
    header.writeUInt32BE(payload.length);
    down.write(Buffer.concat([header, payload]));
  });
  await assert.rejects(
    left.call(
      "start",
      {},
      { timeoutMs: 1000, signal: new AbortController().signal },
    ),
    { message: "IPC closed" },
  );
  assert.equal(left.closed, true);
});
