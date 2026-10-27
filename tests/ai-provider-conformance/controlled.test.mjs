import { PrivateLink } from "../../packages/ai-host/dist/private-link.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { preparedArtifact } from "../../scripts/preparation.mjs";
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
  assertNativeToolSchema,
  executionGeneration,
} from "./support.mjs";

const executable = preparedArtifact("execution");
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
        await openSqliteStore({ path: f.config.databasePath, mode: "open" }),
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
        join(
          preparedArtifact("worker"),
          "bin/node" + (process.platform === "win32" ? ".exe" : ""),
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
        const view = await peer.client.restore(
            (
              await peer.client.createSession({
                sessionId: crypto.randomUUID(),
              })
            ).sessionId,
          ),
          id = view.namespace.sessionId;

        f.model.tool("execution_offers", {}, "catalog-call");
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
        assert.equal(proposals[0].name, "execution_offers");
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
        const policy = JSON.parse(
          await readFile(join(f.directory, "policy-tool.json"), "utf8"),
        );
        f.model.tool(
          policy.name,
          { parameters: { name: "Acme" } },
          "policy-call",
        );
        f.model.text("native policy tool delivered");
        await peer.client.submit(command(id, "policy-projection"));
        await until(
          () =>
            peer.client.getSession(id)?.commands["policy-projection"]?.state ===
            "terminal",
          "policy tool terminal",
        );
        const policyCall = Object.values(peer.client.getSession(id).tools).find(
          (tool) => tool.name === policy.name,
        );
        assert.ok(
          policyCall,
          "actual dynamic Policy MCP call crosses the SDK/Host/Rust boundary",
        );
        assert.deepEqual(policyCall.arguments, {
          parameters: { name: "Acme" },
        });
        assert.equal(policyCall.result.disposition, "rejected");
        assert.equal(JSON.parse(policyCall.result.text).error.code, "denied");
        assert.equal(
          permissions.length,
          0,
          "provider does not add a second Policy confirmation",
        );
        for (const [commandId, choice] of [
          ["execute-no-permission", "execute"],
          // Cancel still uses the provider permission seam; reject before accepting it.
          ["deny-cancel", "reject_once"],
          ["allow-cancel", "allow_once"],
        ]) {
          let detail;
          if (choice !== "execute") {
            const readId = `read-${commandId}`;
            f.model.tool(
              "execution_task_detail",
              {
                reference: {
                  kind: "request",
                  request: result.result[0].request,
                },
              },
              readId,
            );
            f.model.text("original task read");
            await peer.client.submit(command(id, readId));
            await until(
              () =>
                peer.client.getSession(id)?.commands[readId]?.state ===
                "terminal",
              readId,
            );
            const read = Object.values(peer.client.getSession(id).tools).find(
              (tool) =>
                tool.toolCallId === readId ||
                (tool.commandId === readId &&
                  tool.name === "execution_task_detail"),
            );
            assert.equal(read.result.disposition, "returned");
            detail = JSON.parse(read.result.text).result;
          }
          permissionKind = choice;
          const method =
            choice === "execute"
              ? "execution_execute"
              : "execution_task_control";
          f.model.tool(
            method,
            choice === "execute"
              ? (({ request, task, attempt, revision }) => ({
                  request,
                  task,
                  attempt,
                  revision,
                }))(result.result[0])
              : {
                  reference: {
                    kind: "request",
                    request: result.result[0].request,
                  },
                  id: `control-${commandId}`,
                  scope: detail.scope,
                  expectedRevision: detail.revision,
                  action: "cancel",
                },
            commandId,
          );
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
          if (choice === "execute") {
            assert.equal(
              permissions.length,
              0,
              "execution has no generic provider permission",
            );
          } else {
            const request = permissions.at(-1);
            assert.deepEqual(
              request.options.map((option) => option.kind),
              ["allow_once", "reject_once"],
            );
            assert.equal(
              request.toolCall.rawInput.reference.request,
              result.result[0].request,
            );
          }
          const tool = Object.values(peer.client.getSession(id).tools).find(
            (tool) =>
              tool.toolCallId === commandId ||
              (tool.name === method && tool.commandId === commandId),
          );
          assert.ok(tool, "execution result is observable");
          assert.equal(
            tool.result.disposition,
            choice === "reject_once" ? "rejected" : "returned",
          );
        }
        assertNativeToolSchema(provider, f.model.requests, policy);
        assert.equal(permissions.length, 2);
        peer.close();
        assert.equal(await stop(app), 0, stderr);
        const store = unwrap(
          await openSqliteStore({ path: f.config.databasePath, mode: "open" }),
        );
        try {
          const session = unwrap(await store.session(view.namespace));
          assert.equal(activeStage(session).binding.provider, provider);
          assertNativeSession(
            provider,
            session,
            f.model.requests,
            "host_mediated",
            [policy],
          );
        } finally {
          unwrap(await store.close(budget()));
        }
      } catch (error) {
        // Only closed-stage diagnostics; never configuration or tool payloads.
        const diagnostics =
          stderr.match(/RSS_POLICY_DISCOVERY_UNAVAILABLE stage=[a-z-]+/g) ?? [];
        throw new Error(`${error.message}; ${diagnostics.join(", ")}`, {
          cause: error,
        });
      } finally {
        peer?.close();
        const code = await stop(app);
        await stop(rust);
        assert.equal(code, 0, stderr);
      }
    },
  );
}
