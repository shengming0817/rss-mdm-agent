import {
  activeStage,
  startStage,
  providerStage,
} from "@rss-mdm-agent/ai-contract";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  createDeepSeekAdapter,
  type DeepSeekAdapterOptions,
  type DeepSeekConfiguration,
} from "@rss-mdm-agent/ai-adapter-deepseek";
import { VerifiedProviderSession } from "@rss-mdm-agent/ai-contract/session";
import type { ProviderAgentPort, Session } from "@rss-mdm-agent/ai-contract";
const directory = await mkdtemp(join(tmpdir(), "dsh-packed-"));
const config: DeepSeekConfiguration = {
  namespace: {
    tenantId: "consumer-tenant",
    principalId: "user",
    authorityId: "authority",
    sessionId: "session",
  },
  provider: "deepseek",
  config: { id: "official", revision: "1" },

  workingDirectory: directory,
  permissions: "tools_disabled",
};
const options: DeepSeekAdapterOptions = {
  resolveConfiguration: async () => ({
    configuration: config,
    persistenceDirectory: directory,
    endpointIdentity: "https://custom.example.test/v1",
    apiUrl: "https://custom.example.test/v1",
    apiKey: "not-used-no-model-request",
    model: "deepseek-chat",
  }),
};
const first: ProviderAgentPort = createDeepSeekAdapter(options),
  second = createDeepSeekAdapter(options);
const budget = () => ({
  timeoutMs: 10000,
  signal: new AbortController().signal,
});
try {
  const admitted = await VerifiedProviderSession.open(first, config, budget());
  assert.ok(admitted.ok);
  assert.equal((await first.close(budget())).ok, true);
  const previous: Session = startStage(
    {
      schemaVersion: 5,
      kind: "session",
      namespace: config.namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      stages: [],
    },
    providerStage(admitted.value.binding, admitted.value.capabilities),
  );
  const restored = await VerifiedProviderSession.restore(
    second,
    previous,
    config,
    budget(),
  );
  assert.ok(restored.ok);
  assert.ok(restored.value.restores(previous));
  assert.notEqual(
    restored.value.binding.generation,
    activeStage(previous).binding.generation,
  );
  assert.equal(
    restored.value.binding.nativeSessionId,
    activeStage(previous).binding.nativeSessionId,
  );
  console.log(
    "PASS packed public API and real child cold session, no model request",
  );
} finally {
  await first.close(budget());
  await second.close(budget());
  await rm(directory, { recursive: true, force: true });
}
const invalid: DeepSeekAdapterOptions = {
  ...options,
  // @ts-expect-error No test/runtime factory escapes the public adapter interface.
  runtimeFactory: () => null,
};
void invalid;
// @ts-expect-error Provider discrimination survives the fixed tarball boundary.
const wrongProvider: DeepSeekConfiguration = { ...config, provider: "claude" };
void wrongProvider;
