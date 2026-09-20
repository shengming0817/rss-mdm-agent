import { createServer } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import { channelStream } from "../../packages/ai-client/dist/index.js";
import {
  FakeHost,
  fixtureCaller,
  fixtures,
  surfaceCommit,
  readSnapshot,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const root = fileURLToPath(new URL("../../", import.meta.url));
const options = {
  provider: "fake",
  accountRef: "fixture",
  config: { id: "config", revision: "1" },
  profile: "conversation",
};
export async function startFixture() {
  const host = new FakeHost(
    undefined,
    { now: Date.now },
    {
      steer: "supported",
      structuredQuestion: "supported",
      continuation: "same_process",
    },
  );
  const service = createAccessService({
    host,
    now: Date.now,
    sessionOptions: { connectionId: "cfg" },
  });
  const peers = new Map(),
    permissions = [];
  const execution = JSON.parse(
    readFileSync(new URL("./execution-fixtures.json", import.meta.url), "utf8"),
  );
  let executionState = "running";
  const handle = async (req, res, next) => {
    if (!req.url?.startsWith("/__fixture/")) {
      next();
      return;
    }
    try {
      const url = new URL(req.url, "http://fixture"),
        peer = peers.get(url.searchParams.get("peer"));
      res.setHeader("content-type", "application/json");
      if (url.pathname === "/__fixture/connect") {
        const id = randomUUID(),
          state = { queue: [], closed: false };
        state.connection = service.connect(
          channelStream({
            send: async (m) => {
              state.queue.push(m);
            },
            listen(receive, disconnect) {
              state.receive = receive;
              state.disconnect = disconnect;
              return () => {
                state.closed = true;
              };
            },
          }),
          fixtureCaller,
        );
        peers.set(id, state);
        res.end(id);
      } else if (url.pathname === "/__fixture/details") {
        if (
          url.searchParams.get("request") !==
          execution[executionState].status.operationRequestId
        ) {
          res.statusCode = 403;
          res.end("{}");
          return;
        }
        res.end(JSON.stringify(execution[executionState]));
      } else if (!peer || peer.closed) {
        res.statusCode = 410;
        res.end("{}");
      } else if (url.pathname === "/__fixture/receive")
        res.end(JSON.stringify(peer.queue.splice(0)));
      else if (url.pathname === "/__fixture/send") {
        let raw = "";
        for await (const chunk of req) {
          raw += chunk;
          if (raw.length > 1_000_000) throw new Error("fixture budget");
        }
        peer.receive(JSON.parse(raw));
        res.end("{}");
      } else {
        res.statusCode = 404;
        res.end("{}");
      }
    } catch (error) {
      res.statusCode = 500;
      res.end("{}");
      console.error(error);
    }
  };
  const server = await createServer({
    configFile: false,
    root,
    plugins: [
      {
        name: "fixture-api",
        configureServer(server) {
          server.middlewares.use(handle);
        },
      },
      vue(),
    ],
    server: { host: "127.0.0.1", port: 0 },
    logLevel: "error",
  });
  await server.listen();
  const budget = () => ({
    timeoutMs: 5000,
    signal: new AbortController().signal,
  });
  return {
    host,
    service,
    caller: fixtureCaller,
    url: `http://127.0.0.1:${server.httpServer.address().port}/tests/assistant/`,
    execution,
    async seed(count = 23) {
      for (let i = 0; i < count; i++)
        unwrap(await host.openSessionForTest(fixtureCaller, options, budget()));
      unwrap(
        await host.openSessionForTest(
          { ...fixtureCaller, principalId: "other-user" },
          options,
          budget(),
        ),
      );
    },
    setExecution(value) {
      if (!execution[value]) throw new Error("scenario");
      executionState = value;
    },
    async command(sessionId) {
      const s = unwrap(
        await readSnapshot(host.store, { ...fixtureCaller, sessionId }),
      );
      return s.commands.filter((c) => c.command.input.type === "prompt").at(-1)
        .command;
    },
    async question(sessionId, commandId, id = "question") {
      return unwrap(
        await host.ask(
          fixtureCaller,
          sessionId,
          commandId,
          {
            questions: [
              {
                question: "选择下一步",
                header: "诊断",
                options: [
                  { label: "继续检查", description: "只继续对话" },
                  { label: "停止讨论", description: "保留执行结果" },
                ],
                multiSelect: false,
              },
            ],
          },
          id,
        ),
      );
    },
    async surface(sessionId, commandId) {
      const interaction = await this.question(
        sessionId,
        commandId,
        "surface-question",
      );
      const snapshot = unwrap(
        await readSnapshot(host.store, { ...fixtureCaller, sessionId }),
      );
      const surface = {
        ...fixtures.valid.find((row) => row.kind === "surface"),
        namespace: snapshot.session.namespace,
        generation: interaction.generation,
        nativeRunId: interaction.nativeRunId,
        interactionId: interaction.interactionId,
        revision: 0,
        status: "active",
      };
      unwrap(
        await host.store.commit(
          surfaceCommit(snapshot.session, surface, interaction),
        ),
      );
      host.notify(snapshot.session.namespace);
      return surface;
    },
    permission(sessionId) {
      const abort = new AbortController();
      const result = service.requestPermission(
        fixtureCaller,
        {
          sessionId,
          toolCall: { toolCallId: randomUUID(), title: "读取对话资料" },
          options: [
            { optionId: "allow", name: "允许本次", kind: "allow_once" },
            { optionId: "reject", name: "拒绝本次", kind: "reject_once" },
          ],
        },
        abort.signal,
      );
      permissions.push(abort);
      return { abort, result };
    },
    disconnect() {
      for (const peer of peers.values()) {
        peer.disconnect();
        peer.closed = true;
      }
    },
    async close() {
      for (const p of permissions) p.abort();
      await service.close();
      await host.close(budget());
      await server.close();
    },
  };
}
if (process.argv.includes("--serve")) {
  const fixture = await startFixture();
  await fixture.seed();
  console.log(`S1 assistant fixture: ${fixture.url}`);
  for (const signal of ["SIGINT", "SIGTERM"])
    process.once(signal, async () => {
      await fixture.close();
      process.exit(0);
    });
}
