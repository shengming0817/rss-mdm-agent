// Controlled, ephemeral HTTPS RP for the existing target-platform journey. Never use real accounts.
import { createServer } from "node:https";
import { readFileSync } from "node:fs";
import { createHash, createPublicKey, randomBytes, verify } from "node:crypto";
import { pathToFileURL } from "node:url";
import assert from "node:assert/strict";
const digest = (b) => createHash("sha256").update(b).digest();
const bytes = (s) => {
  assert.match(s, /^[A-Za-z0-9_-]+$/);
  return Buffer.from(s, "base64url");
};
// Only the bounded CBOR subset needed for attestation and EC2 COSE keys.
function cbor(input) {
  let offset = 0,
    items = 0;
  function read(depth = 0) {
    assert.ok(depth < 16 && ++items < 4096 && offset < input.length);
    const h = input[offset++],
      major = h >> 5,
      ai = h & 31;
    let n = ai;
    if (ai >= 24) {
      const count = { 24: 1, 25: 2, 26: 4 }[ai];
      assert.ok(count && offset + count <= input.length);
      n = input.readUIntBE(offset, count);
      offset += count;
    }
    if (major === 0) return n;
    if (major === 1) return -1 - n;
    if (major === 2 || major === 3) {
      assert.ok(offset + n <= input.length);
      const b = input.subarray(offset, offset + n);
      offset += n;
      return major === 2 ? b : b.toString("utf8");
    }
    if (major === 4) {
      assert.ok(n < 4096);
      return Array.from({ length: n }, () => read(depth + 1));
    }
    if (major === 5) {
      assert.ok(n < 128);
      const map = new Map();
      for (let i = 0; i < n; i++) {
        const k = read(depth + 1);
        assert.ok(typeof k === "string" || typeof k === "number");
        assert.ok(!map.has(k));
        map.set(k, read(depth + 1));
      }
      return map;
    }
    throw Error("Unsupported CBOR");
  }
  const value = read();
  assert.equal(offset, input.length);
  return value;
}
export class TestRp {
  constructor(origin) {
    this.origin = new URL(origin).origin;
    this.rp = new URL(origin).hostname;
    this.pending = new Map();
    this.credentials = new Map();
  }
  challenge(kind, name = "test-account") {
    assert.ok(kind === "create" || kind === "get");
    assert.ok(this.pending.size < 256 && this.credentials.size < 128);
    assert.ok(typeof name === "string" && name.length > 0 && name.length <= 64);
    const challenge = randomBytes(32).toString("base64url");
    const user = randomBytes(16);
    this.pending.set(challenge, { kind, user, expires: Date.now() + 120000 });
    return kind === "create"
      ? {
          challenge,
          rp: { id: this.rp, name: "RSS controlled RP" },
          user: { id: user.toString("base64url"), name, displayName: name },
          pubKeyCredParams: [{ type: "public-key", alg: -7 }],
          authenticatorSelection: {
            residentKey: "required",
            userVerification: "required",
          },
          attestation: "none",
          timeout: 120000,
        }
      : {
          challenge,
          rpId: this.rp,
          userVerification: "required",
          timeout: 120000,
        };
  }
  accept(kind, input) {
    const client = bytes(input.clientDataJSON);
    assert.ok(client.length <= 16384);
    const data = JSON.parse(client);
    const request = this.pending.get(data.challenge);
    assert.ok(
      request && request.kind === kind && request.expires >= Date.now(),
    );
    this.pending.delete(data.challenge); // A rejected or completed attempt also consumes its challenge.
    assert.equal(
      data.type,
      kind === "create" ? "webauthn.create" : "webauthn.get",
    );
    assert.equal(data.origin, this.origin);
    assert.ok(!data.crossOrigin);
    const id = bytes(input.id);
    assert.ok(id.length <= 1023);
    let auth;
    if (kind === "create") {
      const att = cbor(bytes(input.attestationObject));
      assert.equal(att.get("fmt"), "none");
      auth = att.get("authData");
    } else {
      auth = bytes(input.authenticatorData);
    }
    assert.ok(Buffer.isBuffer(auth) && auth.length >= 37);
    assert.ok(auth.subarray(0, 32).equals(digest(this.rp)));
    assert.equal(auth[32] & 5, 5);
    assert.equal(auth[32] & 0x10, 0); // Local-only candidate is not backed up.
    if (kind === "create") {
      assert.ok((auth[32] & 0x40) !== 0 && auth.length >= 55);
      const length = auth.readUInt16BE(53);
      assert.ok(auth.subarray(55, 55 + length).equals(id));
      const key = cbor(auth.subarray(55 + length));
      assert.equal(key.get(1), 2);
      assert.equal(key.get(3), -7);
      assert.equal(key.get(-1), 1);
      const x = key.get(-2),
        y = key.get(-3);
      assert.ok(
        Buffer.isBuffer(x) &&
          Buffer.isBuffer(y) &&
          x.length === 32 &&
          y.length === 32,
      );
      const publicKey = createPublicKey({
        key: {
          kty: "EC",
          crv: "P-256",
          x: x.toString("base64url"),
          y: y.toString("base64url"),
        },
        format: "jwk",
      });
      assert.ok(!this.credentials.has(input.id));
      this.credentials.set(input.id, { publicKey, user: request.user });
    } else {
      const credential = this.credentials.get(input.id);
      assert.ok(credential);
      assert.ok(bytes(input.userHandle).equals(credential.user));
      assert.ok(
        verify(
          "sha256",
          Buffer.concat([auth, digest(client)]),
          credential.publicKey,
          bytes(input.signature),
        ),
      );
    }
    return { rpAccepted: true, kind };
  }
}
const page = `<!doctype html><meta charset="utf-8"><title>RSS controlled Passkey RP</title>
<h1>RSS Passkey target test</h1><p>Only disposable test accounts. Server state is erased on restart.</p>
<input id="name" value="test-account" maxlength="64"><button id="create">Create</button><button id="get">Authenticate</button><pre id="result"></pre>
<script>
const decode=s=>Uint8Array.from(atob(s.replaceAll('-','+').replaceAll('_','/')),c=>c.charCodeAt(0));
const encode=b=>btoa(String.fromCharCode(...new Uint8Array(b))).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');
async function run(kind){try{
 const options=await fetch('/challenge',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({kind,name:document.querySelector('#name').value})}).then(r=>r.json());
 options.challenge=decode(options.challenge);if(options.user)options.user.id=decode(options.user.id);
 const c=await navigator.credentials[kind]({publicKey:options});const v={id:encode(c.rawId),clientDataJSON:encode(c.response.clientDataJSON)};
 for(const key of ['attestationObject','authenticatorData','signature','userHandle'])if(c.response[key])v[key]=encode(c.response[key]);
 const response=await fetch('/finish',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({kind,value:v})});
 document.querySelector('#result').textContent=JSON.stringify(await response.json(),null,2);
}catch(e){document.querySelector('#result').textContent=JSON.stringify({rpAccepted:false,error:e.name});}}
for(const kind of ['create','get'])document.querySelector('#'+kind).onclick=()=>run(kind);
</script>`;
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const [origin, certificate, key] = process.argv.slice(2);
  const rp = new TestRp(origin);
  assert.equal(new URL(origin).protocol, "https:");
  const server = createServer(
    { cert: readFileSync(certificate), key: readFileSync(key) },
    async (req, res) => {
      res.setHeader("Cache-Control", "no-store");
      try {
        assert.equal(req.headers.host, new URL(origin).host);
        if (req.method === "GET" && req.url === "/") {
          res.setHeader("Content-Type", "text/html; charset=utf-8");
          res.end(page);
          return;
        }
        assert.equal(req.method, "POST");
        assert.equal(req.headers.origin, rp.origin);
        let size = 0;
        const chunks = [];
        for await (const chunk of req) {
          size += chunk.length;
          assert.ok(size <= 131072);
          chunks.push(chunk);
        }
        const input = JSON.parse(Buffer.concat(chunks));
        for (const [id, p] of rp.pending)
          if (p.expires < Date.now()) rp.pending.delete(id);
        const result =
          req.url === "/challenge"
            ? rp.challenge(input.kind, input.name)
            : req.url === "/finish"
              ? rp.accept(input.kind, input.value)
              : null;
        assert.ok(result);
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify(result));
        if (result.rpAccepted) console.log(JSON.stringify(result)); // No credentials, user handles or challenges in logs.
      } catch {
        res.statusCode = 400;
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify({ rpAccepted: false }));
      }
    },
  );
  server.requestTimeout = 125000;
  server.headersTimeout = 10000;
  server.listen(Number(new URL(origin).port) || 443, "127.0.0.1", () =>
    console.log("Controlled RP ready"),
  );
}
