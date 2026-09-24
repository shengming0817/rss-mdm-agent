import { PrivateLink } from "../../packages/ai-host/dist/private-link.js";
import { ConnectionSecrets } from "../../apps/ai-host/dist/secrets.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { once } from "node:events";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { executionServer } from "../ai-host/rust-execution.mjs";
import {
  engines,
  fixture,
  command,
  until,
  unwrap,
  budget,
  clientAt,
  nativePeer,
  capabilities,
  assertNativeSession,
  executionGeneration,
} from "./support.mjs";

const executable = executionServer();
async function stop(child, signal = "SIGTERM") {
  if (child.exitCode !== null || child.signalCode !== null)
    return child.exitCode;
  const exit = once(child, "exit");
  child.kill(signal);
  const timer = setTimeout(() => child.kill("SIGKILL"), 10000);
  try {
    return (await exit)[0];
  } finally {
    clearTimeout(timer);
  }
}
for (const provider of engines) {
  test(
    `${provider}: production controlled-tools admission with real Rust MCP`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      f.config.session.profile = "controlled_tools";
      const catalogStore = unwrap(
        openSqliteStore({ path: f.config.databasePath, mode: "open" }),
      );
      const candidate = {
        ...f.config.connection,
        profile: "controlled_tools",
        configRevision: 2,
      };
      const secrets = new ConnectionSecrets(catalogStore, async () =>
        Buffer.alloc(32, 7),
      );
      unwrap(
        await catalogStore.saveConnection(
          f.config.caller,
          {
            ...f.config.connection,
            profile: "controlled_tools",
            configRevision: 2,
          },
          1,
          await secrets.seal(f.config.caller, candidate, "fixture-only-key"),
        ),
      );
      await catalogStore.close(budget());
      const rust = spawn(
        executable,
        [
          join(f.directory, "execution.sqlite"),
          join(f.directory, "audit.json"),
          "ai-unknown",
        ],
        { stdio: ["pipe", "pipe", "pipe"] },
      );
      // Register ownership before the next spawn can throw.
      t.after(() => stop(rust));
      const app = spawn(
        fileURLToPath(
          new URL(
            "../../.local-ci-runs/worker-runtime/bin/node" +
              (process.platform === "win32" ? ".exe" : ""),
            import.meta.url,
          ),
        ),
        ["apps/ai-host/dist/cli.js", f.path],
        {
          cwd: new URL("../..", import.meta.url),
          stdio: ["pipe", "pipe", "pipe"],
        },
      );
      t.after(() => stop(app));
      const link = new PrivateLink(app.stdout, app.stdin, "native");
      link.lane("execution").pipe(rust.stdin);
      rust.stdout.pipe(link.lane("execution"));
      let stderr = "";
      for (const process of [rust, app]) {
        process.stderr.on("data", (data) => {
          if (stderr.length < 8192) stderr += data;
        });
        process.stdin.on("error", () => {});
      }
      let peer;
      try {
        peer = await clientAt(
          nativePeer(link.lane("native")),
          await executionGeneration(f.directory),
        );
        if (provider !== "codex") {
          const empty = await peer.client.createSession();
          await assert.rejects(
            peer.client.submit(command(empty.namespace.sessionId, "rejected")),
            /unsupported_capability/,
          );
          assert.equal(f.model.requests.length, 0);
          return;
        }
        const view = await peer.client.createSession(),
          id = view.namespace.sessionId;

        f.model.replies.push((res) => {
          const responseId = "catalog-proposal";
          const events = [
            { type: "response.created", response: { id: responseId } },
            {
              type: "response.output_item.done",
              output_index: 0,
              item: {
                type: "function_call",
                call_id: "catalog-call",
                name: "propose",
                namespace: "mcp__rss_host",
                arguments: JSON.stringify({
                  name: "execution_catalog",
                  arguments: {},
                }),
              },
            },
            {
              type: "response.completed",
              response: {
                id: responseId,
                usage: {
                  input_tokens: 1,
                  input_tokens_details: null,
                  output_tokens: 1,
                  output_tokens_details: null,
                  total_tokens: 2,
                },
              },
            },
          ];
          res.writeHead(200, { "content-type": "text/event-stream" });
          for (const event of events)
            res.write(
              `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`,
            );
          res.end();
        });
        f.model.text("catalog read");
        await peer.client.submit(command(id, "catalog"));
        await until(
          () =>
            peer.client.getSession(id)?.commands.catalog?.state === "terminal",
          "catalog terminal",
        );
        const terminal = peer.client.getSession(id);
        assert.deepEqual(
          terminal.capabilities,
          capabilities(provider, "host_mediated"),
        );
        assert.equal(terminal.commands.catalog.outcome, "completed");
        const proposals = Object.values(terminal.tools);
        assert.equal(proposals.length, 1);
        assert.equal(proposals[0].name, "execution_catalog");
        assert.equal(proposals[0].result.disposition, "returned");
        const result = JSON.parse(proposals[0].result.text);
        assert.equal(result.status, "ok");
        assert.ok(result.result, "actual Rust catalog must be returned");
        peer.close();
        assert.equal(await stop(app), 0, stderr);
        const store = unwrap(
          openSqliteStore({ path: f.config.databasePath, mode: "open" }),
        );
        try {
          const session = unwrap(await store.session(view.namespace));
          assert.equal(activeStage(session).binding.providerVersion, "0.155.0");
          assertNativeSession(session, f.model.requests, "host_mediated");
        } finally {
          unwrap(await store.close(budget()));
        }
      } finally {
        peer?.close();
        const code = await stop(app);
        await stop(rust);
        assert.equal(code, 0, stderr);
      }
    },
  );
}
