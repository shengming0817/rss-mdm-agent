import { openHost } from "./host.mjs";
import {
  budget,
  command,
  unwrap,
} from "../ai-provider-conformance/support.mjs";
const [path] = process.argv.slice(2);
let held = false;
const { host, caller } = await openHost(
  path,
  "create",
  async (store, batch) => {
    if (
      !held &&
      batch.commands.some((row) => row.dispatch?.certainty === "submitted")
    ) {
      held = true;
      const session = unwrap(await store.session(batch.namespace));
      const record = unwrap(
        await store.command(batch.namespace, "received-before-commit"),
      );
      process.send({
        type: "before-provider-fact-commit",
        session,
        record,
        launches: unwrap(await store.launches()),
      });
      await new Promise(() => {});
    }
  },
);
const session = unwrap(await host.createSession(caller, {}, budget()));
unwrap(
  await host.submit(
    caller,
    command(session.namespace.sessionId, "received-before-commit"),
    budget(),
  ),
);
