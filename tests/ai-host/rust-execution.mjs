import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

/** Build outside the MCP handshake budget, shared by the real Rust/Host tests. */
export function executionServer() {
  const messages = execFileSync(
    "cargo",
    [
      "build",
      "--locked",
      "--message-format=json",
      "-p",
      "rss-mdm-desktop",
      "--example",
      "execution-acceptance-server",
    ],
    {
      cwd: new URL("../..", import.meta.url),
      encoding: "utf8",
      maxBuffer: 10 * 1024 * 1024,
    },
  )
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  const executable = messages.find(
    (message) =>
      message.reason === "compiler-artifact" &&
      message.target.name === "execution-acceptance-server" &&
      message.executable,
  )?.executable;
  assert.ok(executable, "Cargo must produce the actual acceptance server");
  return executable;
}
