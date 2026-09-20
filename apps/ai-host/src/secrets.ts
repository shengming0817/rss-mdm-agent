import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import type {
  Budget,
  Caller,
  Connection,
  Result,
} from "@rss-mdm-agent/ai-contract";
import { fail } from "@rss-mdm-agent/ai-contract/transitions";
import type { ConnectionSecretStore } from "@rss-mdm-agent/ai-store-sqlite";
import { ConfigurationError } from "./configuration.js";
const value = <T>(result: Result<T>): T => {
  if (!result.ok) throw new ConfigurationError("authentication_required");
  return result.value;
};
const aad = (
  caller: Caller,
  connection: Connection,
  revision = connection.configRevision,
) =>
  Buffer.from(
    JSON.stringify([
      "rss-ai-connection",
      caller.tenantId,
      caller.principalId,
      caller.authorityId,
      connection.connectionId,
      revision,
    ]),
  );
const checked = (secret: string): string => {
  if (!secret.trim() || Buffer.byteLength(secret) > 16384)
    throw new ConfigurationError("authentication_required");
  return secret;
};
/** One app key, fetched lazily. The injected backend is the only native Keychain seam.
 * ref: Node.js crypto createCipheriv/setAAD, AES-256-GCM with 96-bit random IV. */
export class ConnectionSecrets {
  private key?: Promise<Buffer>;
  constructor(
    private readonly store: ConnectionSecretStore,
    private readonly masterKey: (create: boolean) => Promise<Uint8Array>,
  ) {}
  private master(): Promise<Buffer> {
    return (this.key ??= (async () => {
      const key = Buffer.from(
        await this.masterKey(!value(await this.store.hasSecrets())),
      );
      if (key.length !== 32)
        throw new ConfigurationError("authentication_required");
      return key;
    })().catch((error) => {
      this.key = undefined;
      throw error;
    }));
  }
  async read(
    caller: Caller,
    connection: Connection,
    secret?: string,
    revision = connection.configRevision,
  ): Promise<string> {
    if (secret !== undefined) return checked(secret);
    const data = value(
      await this.store.encryptedSecret(
        caller,
        connection.connectionId,
        revision,
      ),
    );
    if (!data || data.length < 29)
      throw new ConfigurationError("authentication_required");
    try {
      const decipher = createDecipheriv(
        "aes-256-gcm",
        await this.master(),
        data.subarray(0, 12),
      );
      decipher.setAuthTag(data.subarray(12, 28));
      decipher.setAAD(aad(caller, connection, revision));
      return checked(
        Buffer.concat([
          decipher.update(data.subarray(28)),
          decipher.final(),
        ]).toString("utf8"),
      );
    } catch {
      throw new ConfigurationError("authentication_required");
    }
  }
  async seal(
    caller: Caller,
    connection: Connection,
    secret: string,
  ): Promise<Uint8Array> {
    const iv = randomBytes(12),
      cipher = createCipheriv("aes-256-gcm", await this.master(), iv);
    cipher.setAAD(aad(caller, connection));
    const encrypted = Buffer.concat([
      cipher.update(checked(secret), "utf8"),
      cipher.final(),
    ]);
    return Buffer.concat([iv, cipher.getAuthTag(), encrypted]);
  }
}

/** The application-owned atomic persistence seam: validate/reencrypt first, then
 * recheck the native caller fence immediately before the SQLite CAS. */
export function connectionPersistence(
  store: ConnectionSecretStore,
  secrets: ConnectionSecrets,
  available: (caller: Caller) => boolean,
) {
  return async (
    caller: Caller,
    connection: Connection,
    expected: number | null,
    secret: string | undefined,
    budget: Budget,
  ): Promise<Result<Connection>> => {
    const encrypted =
      connection.status !== "deleted" && connection.source.type === "custom_api"
        ? await secrets.seal(
            caller,
            connection,
            await secrets.read(caller, connection, secret, expected ?? 0),
          )
        : undefined;
    if (!available(caller) || budget.signal.aborted) return fail("unavailable");
    return store.saveConnection(caller, connection, expected, encrypted);
  };
}
