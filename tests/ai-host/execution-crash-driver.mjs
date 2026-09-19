import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { Deliveries } from "../../packages/ai-host/dist/delivery.js";
import {
  acceptance,
  dispatchCommand,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

const [path, sessionJson, commandJson, proposalJson, preparedJson] =
  process.argv.slice(2);
const session = JSON.parse(sessionJson);
const command = JSON.parse(commandJson);
const proposal = JSON.parse(proposalJson);
const prepared = JSON.parse(preparedJson);
const store = unwrap(openSqliteStore({ path, mode: "create" }));
unwrap(await store.create(session));
unwrap(
  await store.accept({
    ...acceptance(session, command),
    nowMs: Date.now(),
    retention: { retryWindowMs: 60_000, receiptWindowMs: 60_000 },
  }),
);
await dispatchCommand(
  store,
  unwrap(await store.session(session.namespace)),
  command.commandId,
  "submitted",
);
const current = unwrap(await store.session(session.namespace));
let tail = Promise.resolve();
const deliveries = new Deliveries(
  store,
  {
    prepare: () => ({ ok: true, value: prepared }),
    send: async (request) => {
      process.send({ type: "send", request });
      await new Promise(() => {});
    },
    reconcile: async () => ({ ok: true, value: { state: "unknown" } }),
    acknowledge: async () => ({ ok: true, value: undefined }),
  },
  (_, action) => {
    const task = tail.then(action);
    tail = task.catch(() => {});
    return task;
  },
  async () => {},
);
unwrap(
  await deliveries.propose(
    session.namespace,
    current.binding.generation,
    command.commandId,
    proposal,
    { timeoutMs: 60_000, signal: new AbortController().signal },
  ),
);
