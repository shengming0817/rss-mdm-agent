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
  emptyCommit,
  fixtureLimits,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
import {
  activeStage,
  deliveryFingerprint,
} from "../../packages/ai-contract/dist/index.js";
const root = fileURLToPath(new URL("../../", import.meta.url));
const options = {
  provider: "fake",

  config: { id: "config", revision: "1" },
  profile: "conversation",
};
export async function createFixture() {
  const host = new FakeHost(
    undefined,
    { now: Date.now },
    {
      steer: "supported",
      structuredQuestion: "supported",
      continuation: "same_process",
    },
  );
  unwrap(
    await host.store.saveConnection(
      fixtureCaller,
      {
        schemaVersion: 7,
        kind: "connection",
        connectionId: "cfg",
        name: "Browser fixture",
        provider: "codex",
        configRevision: 1,

        status: "ready",
        profile: "conversation",
        source: {
          type: "custom_api",
          apiUrl: "https://example.invalid",
          model: "fixture",
        },
      },
      null,
    ),
  );
  unwrap(
    await host.store.savePreferences(fixtureCaller, {
      defaultConnectionId: { set: "cfg" },
    }),
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
  const serviceViews = JSON.parse(
    readFileSync(new URL("./service-fixtures.json", import.meta.url), "utf8"),
  );
  let executionState = "running",
    scenario = "running";
  const scenarios = [
    ...Object.keys(serviceViews),
    "proposed",
    "denied",
    "running",
    "software",
    "cancelling",
    "outcomeUnknown",
    "verified",
    "cancelled",
  ];
  function selectScenario(value) {
    if (!scenarios.includes(value)) throw new Error("unknown fixture scenario");
    scenario = value;
    if (execution[value]?.status) executionState = value;
  }
  function serviceView() {
    return structuredClone(serviceViews[scenario] ?? serviceViews.ready);
  }
  function available() {
    const view = serviceView();
    if (view.phase !== "connected" || view.status.readiness.phase !== "ready")
      throw new Error("fixture service unavailable");
  }
  function details(request) {
    available();
    if (request !== execution.offer.request)
      throw new Error("fixture request mismatch");
    return ["proposed", "denied"].includes(scenario)
      ? { kind: "pending", value: structuredClone(execution[scenario]) }
      : {
          kind: "execution",
          value: structuredClone(execution[executionState]),
        };
  }
  function snapshot(input = {}) {
    available();
    return {
      available: [structuredClone(execution.offer)],
      preparations: ["proposed", "denied"].includes(scenario)
        ? [structuredClone(execution[scenario])]
        : [],
      selected: input.selected ? details(input.selected) : null,
      requests: ["proposed", "denied", "ready"].includes(scenario)
        ? []
        : [structuredClone(execution[executionState])],
      next: null,
    };
  }
  function execute(input) {
    available();
    const offer = execution.offer;
    if (
      !["request", "task", "attempt", "revision"].every(
        (key) => input?.[key] === offer[key],
      )
    )
      throw new Error("fixture selection mismatch");
    if (scenario === "denied") throw new Error("fixture request denied");
    if (["proposed", "ready"].includes(scenario)) {
      scenario = "running";
      executionState = "running";
    }
    return { request: offer.request, confirmationRequired: false };
  }
  function cancel(input) {
    available();
    if (input?.requestId !== execution.offer.request)
      throw new Error("fixture request mismatch");
    if (!["cancelled", "outcomeUnknown", "verified"].includes(scenario)) {
      scenario = "cancelling";
      executionState = "cancelling";
    }
    return details(input.requestId);
  }

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
      } else if (
        [
          "/__fixture/state",
          "/__fixture/scenario",
          "/__fixture/snapshot",
          "/__fixture/execute",
          "/__fixture/cancel",
        ].includes(url.pathname)
      ) {
        let input = {};
        if (req.method === "POST") {
          let raw = "";
          for await (const chunk of req) {
            raw += chunk;
            if (raw.length > 65536) throw new Error("fixture budget");
          }
          input = JSON.parse(raw || "{}");
        }
        let value;
        switch (url.pathname) {
          case "/__fixture/state":
            value = { scenario, scenarios, service: serviceView() };
            break;
          case "/__fixture/scenario":
            selectScenario(input.scenario);
            value = {};
            break;
          case "/__fixture/snapshot":
            value = snapshot(input);
            break;
          case "/__fixture/execute":
            value = execute(input);
            break;
          case "/__fixture/cancel":
            value = cancel(input);
            break;
        }
        res.end(JSON.stringify(value));
      } else if (
        url.pathname === "/__fixture/visual-delivery" &&
        req.method === "POST"
      ) {
        const sessions = unwrap(
          await host.store.listSessions(fixtureCaller, { limit: 50 }),
        );
        const sessionId = sessions.items.find((row) =>
          row.title.startsWith("原生视觉验收"),
        ).namespace.sessionId;
        const command = await fixture.command(sessionId);
        await fixture.executionDelivery(sessionId, command.commandId);
        unwrap(
          await host.advance(fixtureCaller, sessionId, command.commandId, [
            { type: "terminal", outcome: "completed" },
          ]),
        );
        res.end("{}");
      } else if (url.pathname === "/__fixture/details") {
        if (
          url.searchParams.get("request") !==
          execution[executionState].status.operationRequestId
        ) {
          res.statusCode = 403;
          res.end("{}");
          return;
        }
        res.end(JSON.stringify(details(url.searchParams.get("request"))));
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
      console.error(`fixture request failed: ${error.message}`);
    }
  };
  const plugin = {
    name: "fixture-api",
    configureServer(server) {
      server.middlewares.use(handle);
    },
  };
  const budget = () => ({
    timeoutMs: 5000,
    signal: new AbortController().signal,
  });
  const fixture = {
    plugin,
    snapshot,
    execute,
    cancel,
    details,
    serviceView,
    selectScenario,
    host,
    service,
    caller: fixtureCaller,
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
      scenario = value;
    },
    // Test-only association: phase/effect facts remain the generated Rust fixture.
    async executionDelivery(
      sessionId,
      commandId,
      operationId = "visual-execution",
    ) {
      await this.host.advance(fixtureCaller, sessionId, commandId, []);
      const namespace = { ...fixtureCaller, sessionId };
      const session = unwrap(await host.store.session(namespace));
      const event = {
        schemaVersion: 7,
        kind: "event",
        namespace,
        eventId: `visual-${operationId}`,
        sequence: session.lastSequence + 1,
        generation: activeStage(session).binding.generation,
        commandId,
        body: {
          type: "delivery_requested",
          operationId,
          target: "rust-execution",
          proposal: {
            name: "execution_execute",
            arguments: {
              request: execution.running.status.operationRequestId,
              task: "visual-task",
              attempt: "visual-attempt",
              revision: "a".repeat(64),
            },
          },
        },
      };
      const row = {
        schemaVersion: 7,
        kind: "delivery",
        namespace,
        operationId,
        eventId: event.eventId,
        target: event.body.target,
        contentHash: deliveryFingerprint(
          event,
          event.body.target,
          fixtureLimits,
        ),
        retry: "receiver_idempotent",
        status: "pending",
        attempts: 0,
        nextAttemptAtMs: 0,
      };
      const commit = {
        ...emptyCommit(session),
        session: {
          ...session,
          revision: session.revision + 1,
          lastSequence: event.sequence,
        },
        events: [event],
        deliveries: [row],
      };
      unwrap(await host.store.commit(commit));
      for (const value of Object.values(execution).filter((row) => row.action))
        value.action.initiator = {
          kind: "ai",
          provider: "test-fixture",
          config: { id: "fixture", revision: "1" },
          conversation: sessionId,
          toolCall: operationId,
          osSession: {
            device: "fixture-device",
            session: "fixture-os-session",
            account: { platform: "linux", subject: "fixture-account" },
          },
        };
      // Wake the existing subscription after the host-owned commit.
      unwrap(
        await host.advance(fixtureCaller, sessionId, commandId, [
          {
            type: "text",
            messageId: "visual-execution-note",
            text: "设备状态由独立执行记录呈现。S1 测试投影不代表设备变更。",
          },
        ]),
      );
      return operationId;
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
    },
  };
  return fixture;
}
export async function startFixture() {
  const fixture = await createFixture();
  const server = await createServer({
    configFile: false,
    root,
    plugins: [fixture.plugin, vue()],
    server: { host: "127.0.0.1", port: 0 },
    logLevel: "error",
  });
  await server.listen();
  return {
    ...fixture,
    url: `http://127.0.0.1:${server.httpServer.address().port}/tests/assistant/`,
    async close() {
      await fixture.close();
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
