import test from "node:test";
import assert from "node:assert/strict";
import { passkeyEntitlements } from "./build-macos-passkey.mjs";
test("Passkey host capability merge preserves PSSO and deduplicates shared access", () => {
  const existing = {
    "com.apple.developer.associated-domains.mdm-managed": true,
    "com.apple.developer.associated-domains": ["webcredentials:example.test"],
    "com.apple.security.application-groups": ["group.other"],
  };
  const result = passkeyEntitlements(existing);
  assert.deepEqual(
    result["com.apple.developer.associated-domains"],
    existing["com.apple.developer.associated-domains"],
  );
  assert.equal(
    result["com.apple.developer.associated-domains.mdm-managed"],
    true,
  );
  assert.equal(
    result[
      "com.apple.developer.authentication-services.autofill-credential-provider"
    ],
    true,
  );
  assert.deepEqual(passkeyEntitlements(result), result);
  assert.deepEqual(existing["com.apple.security.application-groups"], [
    "group.other",
  ]);
});
