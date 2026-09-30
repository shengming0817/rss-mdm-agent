import { PrivateLink } from "../../packages/ai-host/dist/private-link.js";
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
      unwrap(
        await catalogStore.saveConnection(
          f.config.caller,
          {
            ...f.config.connection,
            profile: "controlled_tools",
            configRevision: 2,
          },
          1,
          new Uint8Array(32).fill(7),
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
      const permissions = [];
      let permissionKind = "allow_once";
      try {
        peer = await clientAt(
          nativePeer(link.lane("native")),
          await executionGeneration(f.directory),
          {
            requestPermission: async (request) => {
              permissions.push(request);
              return {
                outcome: {
                  outcome: "selected",
                  optionId: request.options.find(
                    (option) => option.kind === permissionKind,
                  ).optionId,
                },
              };
            },
          },
        );
        if (provider !== "codex") {
          const empty = await peer.client.restore(
            (
              await peer.client.createSession({
                sessionId: crypto.randomUUID(),
              })
            ).sessionId,
          );
          await assert.rejects(
            peer.client.submit(command(empty.namespace.sessionId, "rejected")),
            /unsupported_capability/,
          );
          assert.equal(f.model.requests.length, 0);
          peer.close();
          assert.equal(await stop(app), 0, stderr);
          const store = unwrap(
            openSqliteStore({ path: f.config.databasePath, mode: "open" }),
          );
          try {
            const rejected = unwrap(await store.session(empty.namespace));
            assert.deepEqual(rejected.stages, []);
            assert.equal(rejected.currentStageId, undefined);
          } finally {
            unwrap(await store.close(budget()));
          }
          return;
        }
        const view = await peer.client.restore(
            (
              await peer.client.createSession({
                sessionId: crypto.randomUUID(),
              })
            ).sessionId,
          ),
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
                  name: "execution_tasks",
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
        await peer.client.submit(
          command(id, "catalog", "GOLDEN_INSTALL 安装办公套件"),
        );
        await peer.client.listSessions({ limit: 20 });
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
        assert.equal(proposals[0].name, "execution_tasks");
        assert.equal(
          proposals[0].result.disposition,
          "returned",
          JSON.stringify(proposals[0].result),
        );
        const result = JSON.parse(proposals[0].result.text);
        assert.equal(result.status, "ok");
        assert.ok(
          result.result,
          "actual Rust task references must be returned",
        );
        assert.equal(
          permissions.length,
          0,
          "catalog reads never ask permission",
        );
        for (const [commandId, choice] of [
          // Reject first: once this exact backend request is accepted, replay must return
          // its original receipt instead of asking a contradictory second permission.
          ["deny-execute", "reject_once"],
          ["allow-execute", "allow_once"],
        ]) {
          permissionKind = choice;
          f.model.replies.push((res) => {
            const events = [
              { type: "response.created", response: { id: commandId } },
              {
                type: "response.output_item.done",
                output_index: 0,
                item: {
                  type: "function_call",
                  call_id: commandId,
                  name: "propose",
                  namespace: "mcp__rss_host",
                  arguments: JSON.stringify({
                    name: "execution_execute",
                    arguments: (({ request, task, attempt, revision }) => ({
                      request,
                      task,
                      attempt,
                      revision,
                    }))(result.result[0]),
                  }),
                },
              },
              {
                type: "response.completed",
                response: {
                  id: commandId,
                  usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 },
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
          f.model.text("execution decision recorded");
          await peer.client.submit(command(id, commandId));
          await until(
            () =>
              peer.client.getSession(id)?.commands[commandId]?.state ===
              "terminal",
            commandId,
          );
          assert.equal(
            peer.client.getSession(id).commands[commandId].outcome,
            "completed",
            stderr,
          );
          const request = permissions.at(-1);
          assert.deepEqual(
            request.options.map((option) => option.kind),
            ["allow_once", "reject_once"],
          );
          assert.equal(
            request.toolCall.rawInput.request,
            result.result[0].request,
          );
          const tool = Object.values(peer.client.getSession(id).tools).find(
            (tool) =>
              tool.toolCallId === commandId ||
              (tool.name === "execution_execute" &&
                tool.commandId === commandId),
          );
          assert.ok(tool, "execution result is observable");
          assert.equal(
            tool.result.disposition,
            choice === "allow_once" ? "returned" : "rejected",
          );
        }
        assert.equal(permissions.length, 2);
        peer.close();
        assert.equal(await stop(app), 0, stderr);
        const store = unwrap(
          openSqliteStore({ path: f.config.databasePath, mode: "open" }),
        );
        try {
          const session = unwrap(await store.session(view.namespace));
          assert.equal(activeStage(session).binding.providerVersion, "0.155.0");
          assertNativeSession(
            provider,
            session,
            f.model.requests,
            "host_mediated",
          );
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
