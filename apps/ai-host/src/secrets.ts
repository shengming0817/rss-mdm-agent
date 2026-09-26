import type {
  Caller,
  Connection,
  ConnectionDraft,
  SessionStore,
} from "@rss-mdm-agent/ai-contract";
import type { CredentialPersistence } from "@rss-mdm-agent/ai-host";
import { fail } from "@rss-mdm-agent/ai-contract/transitions";
import type { ConnectionSecretStore } from "@rss-mdm-agent/ai-store-sqlite";
import { ConfigurationError } from "./configuration.js";

/** Strip session-only fields without deriving or defaulting identity. */
function credentialCaller({
  tenantId,
  principalId,
  authorityId,
}: Caller): Caller {
  return { tenantId, principalId, authorityId };
}
/** Project editable fields only; native code owns all credential binding semantics. */
function credentialDraft(connection: Connection): ConnectionDraft {
  const { connectionId, name, provider, profile, source } = connection;
  return { connectionId, name, provider, profile, source };
}
/** Rust owns the key and cryptography. Only activation consumes decrypted material. */
export class ConnectionSecrets {
  constructor(
    private readonly store: ConnectionSecretStore,
    private readonly open: (
      caller: Caller,
      connection: ConnectionDraft,
      encrypted: number[],
    ) => Promise<unknown>,
  ) {}
  async read(caller: Caller, connection: Connection): Promise<string> {
    const stored = await this.store.encryptedSecret(
      caller,
      connection.connectionId,
      connection.configRevision,
    );
    if (!stored.ok || !stored.value)
      throw new ConfigurationError("authentication_required");
    let secret: unknown;
    try {
      secret = await this.open(
        credentialCaller(caller),
        credentialDraft(connection),
        [...stored.value],
      );
    } catch {
      throw new ConfigurationError("authentication_required");
    }
    if (
      typeof secret !== "string" ||
      !secret.length ||
      Buffer.byteLength(secret) > 16384
    )
      throw new ConfigurationError("authentication_required");
    return secret;
  }
}

/** Async preparation is outside SQLite; the final write rechecks revision and caller. */
export function connectionPersistence(
  store: ConnectionSecretStore & Pick<SessionStore, "connection">,
  available: (caller: Caller) => boolean,
  matches: (
    caller: Caller,
    previous: ConnectionDraft,
    connection: ConnectionDraft,
  ) => Promise<boolean>,
): CredentialPersistence {
  return async (caller, connection, expected, credential, budget) => {
    let encrypted =
      credential.type === "replace" ? credential.encrypted : undefined;
    if (
      connection.status !== "deleted" &&
      connection.source.type === "custom_api"
    ) {
      if (encrypted === undefined) {
        if (expected === null) return fail("authentication_required");
        const previous = await store.connection(
          caller,
          connection.connectionId,
          expected,
        );
        if (!previous.ok) return previous;
        if (
          previous.value.source.type !== "custom_api" ||
          !(await matches(
            credentialCaller(caller),
            credentialDraft(previous.value),
            credentialDraft(connection),
          ))
        )
          return fail("authentication_required");
        const stored = await store.encryptedSecret(
          caller,
          connection.connectionId,
          expected,
        );
        if (!stored.ok || !stored.value) return fail("authentication_required");
        encrypted = stored.value;
      }
    } else {
      if (encrypted !== undefined) return fail("invalid_input");
    }
    if (!available(caller) || budget.signal.aborted) return fail("unavailable");
    return store.saveConnection(caller, connection, expected, encrypted);
  };
}
