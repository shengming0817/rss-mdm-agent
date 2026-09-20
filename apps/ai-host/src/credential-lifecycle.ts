import type {
  Caller,
  Connection,
  SessionStore,
} from "@rss-mdm-agent/ai-contract";
import { isSettled } from "@rss-mdm-agent/ai-contract/transitions";
import type { LocalConfiguration } from "./configuration.js";
import { nativeContext, nativeCredential } from "./credentials.js";

/** The Host catalog and unresolved receipts own retention; native owns only secret storage. */
export function credentialLifecycle(
  local: LocalConfiguration,
  store: SessionStore,
) {
  const request = async (caller: Caller, action: object) => {
    const context = await nativeContext(local.usersPath, caller);
    await nativeCredential(local.credentialSocket, {
      ...action,
      userId: caller.principalId,
      generation: context.generation,
    });
  };
  return {
    async activate(caller: Caller, connection: Connection) {
      if (connection.source.type === "custom_api")
        await request(caller, {
          type: "activate",
          credentialRef: connection.credentialRef,
        });
    },
    async discard(caller: Caller, connection: Connection) {
      if (connection.source.type === "custom_api")
        await request(caller, {
          type: "discard",
          credentialRef: connection.credentialRef,
        });
    },
    async collect(caller: Caller) {
      const keep = new Set<string>();
      const catalog = await store.connections(caller);
      if (!catalog.ok) throw new Error("credential cleanup unavailable");
      for (const row of catalog.value)
        if (row.source.type === "custom_api") keep.add(row.credentialRef);
      let continuation: string | undefined;
      do {
        const sessions = await store.listSessions(caller, {
          limit: 256,
          ...(continuation ? { continuation } : {}),
        });
        if (!sessions.ok) throw new Error("credential cleanup unavailable");
        for (const session of sessions.value.items) {
          let next: string | undefined;
          do {
            const page = await store.snapshotPage(session.namespace, {
              limit: 256,
              ...(next ? { continuation: next } : {}),
            });
            if (!page.ok) throw new Error("credential cleanup unavailable");
            for (const row of page.value.commands) {
              if (isSettled(row)) continue;
              const stage = page.value.session.stages.find(
                (stage) => stage.stageId === row.receipt.stageId,
              );
              if (!stage) throw new Error("credential cleanup unavailable");
              const revision = await store.connection(
                caller,
                stage.connectionId,
                stage.configRevision,
              );
              if (!revision.ok)
                throw new Error("credential cleanup unavailable");
              if (revision.value.source.type === "custom_api")
                keep.add(revision.value.credentialRef);
            }
            next = page.value.next;
          } while (next);
        }
        continuation = sessions.value.next;
      } while (continuation);
      await request(caller, { type: "collect", keep: [...keep] });
    },
  };
}
