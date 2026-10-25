import test from "node:test";
import assert from "node:assert/strict";
import {
  generateKeyPairSync,
  createHash,
  sign,
  randomBytes,
} from "node:crypto";
import { TestRp } from "../tests/passkey-provider/rp.mjs";
test("controlled RP independently verifies signatures, origin, user handle and consumes challenges", () => {
  const rp = new TestRp("https://passkey.example.test");
  const { privateKey, publicKey } = generateKeyPairSync("ec", {
    namedCurve: "prime256v1",
  });
  const user = randomBytes(16),
    id = randomBytes(32).toString("base64url");
  rp.credentials.set(id, { publicKey, user });
  function reply(options, origin = rp.origin) {
    const client = Buffer.from(
      JSON.stringify({
        type: "webauthn.get",
        origin,
        challenge: options.challenge,
      }),
    );
    const auth = Buffer.concat([
      createHash("sha256").update(rp.rp).digest(),
      Buffer.from([0x0d, 0, 0, 0, 0]),
    ]);
    const signature = sign(
      "sha256",
      Buffer.concat([auth, createHash("sha256").update(client).digest()]),
      privateKey,
    );
    return {
      id,
      clientDataJSON: client.toString("base64url"),
      authenticatorData: auth.toString("base64url"),
      signature: signature.toString("base64url"),
      userHandle: user.toString("base64url"),
    };
  }
  const first = reply(rp.challenge("get"));
  assert.equal(rp.accept("get", first).rpAccepted, true);
  assert.throws(() => rp.accept("get", first));
  assert.throws(() =>
    rp.accept("get", reply(rp.challenge("get"), "https://evil.example.test")),
  );
  const wrong = reply(rp.challenge("get"));
  wrong.userHandle = randomBytes(16).toString("base64url");
  assert.throws(() => rp.accept("get", wrong));
  const forged = reply(rp.challenge("get"));
  forged.signature = randomBytes(64).toString("base64url");
  assert.throws(() => rp.accept("get", forged));
});
