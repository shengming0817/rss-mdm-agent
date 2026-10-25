import test from "node:test";
import assert from "node:assert/strict";
import { identityManifest, unsignedPublisher } from "./passkey-package.mjs";
test("identity-only carrier is bound to the native COM executable and keeps Vault outside package storage", () => {
  const manifest = identityManifest({
    version: "0.1.6.0",
    publisher: unsignedPublisher,
  });
  assert.match(manifest, /AllowExternalContent>true/);
  assert.match(
    manifest,
    /Executable="rss-passkey-provider.exe" Arguments="-Embedding"/,
  );
  assert.match(manifest, /unvirtualizedResources/);
  assert.doesNotMatch(manifest, /windows.service|localSystemServices/);
  assert.match(manifest, /OID.2.25.311729368913984317654407730594956997722=1/);
});
test("invalid package identity is rejected before invoking Windows build tools", () => {
  assert.throws(() =>
    identityManifest({ version: "1.0.0", publisher: "CN=RSS" }),
  );
  assert.throws(() =>
    identityManifest({ version: "1.0.0.65536", publisher: "CN=RSS" }),
  );
  assert.throws(() =>
    identityManifest({ version: "1.0.0.0", publisher: "CN=RSS\n" }),
  );
  assert.match(
    identityManifest({ version: "1.0.0.0", publisher: "CN=RSS & Test" }),
    /Publisher="CN=RSS &amp; Test"/,
  );
});
