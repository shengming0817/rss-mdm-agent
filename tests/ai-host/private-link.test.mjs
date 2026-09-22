import test from "node:test";
import assert from "node:assert/strict";
import { PassThrough, Writable } from "node:stream";
import { PrivateLink } from "../../packages/ai-host/dist/private-link.js";

test("fixed lanes demultiplex fragmented frames and close together", async () => {
  const input = new PassThrough(),
    output = new PassThrough();
  const remoteInput = new PassThrough(),
    remoteOutput = new PassThrough();
  output.on("data", (b) => {
    for (const byte of b) remoteInput.write(Buffer.from([byte]));
  });
  remoteOutput.on("data", (b) => input.write(b));
  const a = new PrivateLink(input, output, "native");
  const b = new PrivateLink(remoteInput, remoteOutput, "native");
  const got = new Promise((resolve) => b.lane("native").once("data", resolve));
  a.lane("native").write(Buffer.from("control"));
  assert.equal((await got).toString(), "control");
  assert.throws(() => a.lane("tools"));
  a.close();
  b.close();
});

test("control preempts queued output and a blocked physical writer stays bounded", async () => {
  const input = new PassThrough(),
    frames = [];
  const output = new Writable({
    highWaterMark: 1,
    write(chunk, _, done) {
      frames.push(Buffer.from(chunk));
      done();
    },
  });
  const link = new PrivateLink(input, output, "worker");
  link.lane("events").write(Buffer.from("bulk"));
  link.lane("control").write(Buffer.from("cancel"));
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(frames[0][4], 0);
  assert.equal(frames[1][4], 2);
  link.close();
  const blocked = new PrivateLink(
    new PassThrough(),
    new Writable({ highWaterMark: 1, write() {} }),
    "worker",
  );
  blocked.lane("events").write(Buffer.alloc(512 * 1024));
  await new Promise((resolve) => setImmediate(resolve));
  for (let i = 0; i < 3; i++)
    blocked.lane("events").write(Buffer.alloc(512 * 1024));
  assert.equal(blocked.closed, true);
});

test("legacy, unknown version and oversized framing closes before dispatch", () => {
  for (const bytes of [
    Buffer.from('{"schemaVersion":5}\n'),
    Buffer.from([82, 83, 83, 0, 0, 0, 0, 1, 0]),
    Buffer.from([82, 83, 83, 1, 0, 255, 255, 255, 255]),
  ]) {
    const input = new PassThrough(),
      output = new PassThrough();
    const link = new PrivateLink(input, output, "native");
    let calls = 0;
    link.lane("native").on("data", () => calls++);
    input.write(bytes);
    assert.equal(link.closed, true);
    assert.equal(calls, 0);
  }
});
