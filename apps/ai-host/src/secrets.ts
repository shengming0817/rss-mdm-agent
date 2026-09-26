import type {
  Budget,
  Caller,
  Connection,
  ConnectionDraft,
  CredentialOwner,
  Result,
  SessionStore,
} from "@rss-mdm-agent/ai-contract";
import { fail } from "@rss-mdm-agent/ai-contract/transitions";
import type { ConnectionSecretStore } from "@rss-mdm-agent/ai-store-sqlite";
import { ConfigurationError } from "./configuration.js";

/** Stable target binding: configuration-only revisions retain the exact ciphertext. */
export function credentialOwner(
  caller: Caller,
  connection: Connection | ConnectionDraft,
): CredentialOwner {
  if (connection.source.type !== "custom_api")
    throw new ConfigurationError("configuration_invalid");
  return {
    tenantId: caller.tenantId,
    principalId: caller.principalId,
    authorityId: caller.authorityId,
    connectionId: connection.connectionId,
    provider: connection.provider,
    endpoint: new URL(connection.source.apiUrl).href,
    credentialType: connection.source.credentialType ?? "api_key",
  };
}
/** Rust owns the key and cryptography. Only activation consumes decrypted material. */
export class ConnectionSecrets {
  constructor(
    private readonly store: ConnectionSecretStore,
    private readonly open: (
      owner: CredentialOwner,
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
      secret = await this.open(credentialOwner(caller, connection), [
        ...stored.value,
      ]);
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
) {
  return async (
    caller: Caller,
    connection: Connection,
    expected: number | null,
    encrypted: Uint8Array | undefined,
    budget: Budget,
  ): Promise<Result<Connection>> => {
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
          JSON.stringify(credentialOwner(caller, previous.value)) !==
            JSON.stringify(credentialOwner(caller, connection))
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
