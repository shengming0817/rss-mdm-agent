import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import { mkdtemp, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createProvider } from "../../apps/ai-host/dist/provider.js";
import {
  fixtureSession,
  fixtureCommand,
  fixtureAttempt,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
import {
  resolvePinnedLookup,
  startEgressProxy,
} from "../../apps/ai-host/dist/egress.js";

const listen = async (server) => {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return `http://127.0.0.1:${server.address().port}`;
};

test("approved DNS answers are pinned into the actual socket lookup", async () => {
  let resolutions = 0;
  const pinned = await resolvePinnedLookup(
    new URL("https://provider.example/v1"),
    async () => {
      resolutions++;
      return [{ address: "93.184.216.34", family: 4 }];
    },
  );
  const inspect = () =>
    new Promise((resolve, reject) =>
      pinned("provider.example", {}, (error, address, family) =>
        error ? reject(error) : resolve({ address, family }),
      ),
    );
  assert.deepEqual(await inspect(), {
    address: "93.184.216.34",
    family: 4,
  });
  assert.deepEqual(await inspect(), {
    address: "93.184.216.34",
    family: 4,
  });
  assert.equal(resolutions, 1, "the socket must not resolve the host again");
});

test("public HTTPS egress rejects any private DNS answer", async () => {
  await assert.rejects(
    resolvePinnedLookup(new URL("https://provider.example/v1"), async () => [
      { address: "93.184.216.34", family: 4 },
      { address: "169.254.169.254", family: 4 },
    ]),
    /egress_rejected/,
  );
});

test("the fixed proxy refuses redirects before credentials reach another origin", async (t) => {
  let leakedAuthorization;
  const destination = createServer((request, response) => {
    leakedAuthorization = request.headers.authorization;
    response.end("unexpected");
  });
  const destinationUrl = await listen(destination);
  const redirector = createServer((_request, response) => {
    response.writeHead(307, { location: destinationUrl });
    response.end();
  });
  const baseUrl = await listen(redirector);
  const proxy = await startEgressProxy(baseUrl);
  t.after(async () => {
    await proxy.close();
    destination.closeAllConnections();
    redirector.closeAllConnections();
    await Promise.all([
      new Promise((resolve) => destination.close(resolve)),
      new Promise((resolve) => redirector.close(resolve)),
    ]);
  });
  const response = await fetch(`${proxy.endpoint}/chat/completions`, {
    method: "POST",
    headers: { authorization: "Bearer cloud-secret" },
    body: "{}",
  });
  assert.equal(response.status, 502);
  assert.equal(leakedAuthorization, undefined);
});

test("egress consumes HTTP evidence only for its dispatch and never reuses authentication failure", async (t) => {
  let status = 401;
  const server = createServer((_req, res) =>
    res.writeHead(status).end("CANARY_SECRET_PRIVATE_PATH"),
  );
  const proxy = await startEgressProxy(await listen(server));
  t.after(async () => {
    await proxy.close();
    server.closeAllConnections();
    await new Promise((r) => server.close(r));
  });
  for (const [http, expected] of [
    [401, "authentication_required"],
    [403, "permission_denied"],
    [429, "limit_exceeded"],
    [500, "unavailable"],
    [200, undefined],
  ]) {
    status = http;
    proxy.beginAttempt(String(http));
    await (await fetch(proxy.endpoint)).text();
    assert.equal(proxy.takeFailure(String(http)), expected);
    assert.equal(proxy.takeFailure(String(http)), undefined);
    proxy.endAttempt(String(http));
    assert.equal(JSON.stringify(proxy).includes("CANARY"), false);
  }
  status = 401;
  proxy.beginAttempt("old");
  await (await fetch(proxy.endpoint)).text();
  proxy.endAttempt("old");
  await new Promise((r) => server.close(r));
  proxy.beginAttempt("network");
  assert.equal((await fetch(proxy.endpoint)).status, 502);
  assert.equal(proxy.takeFailure("network"), "unavailable");
  proxy.endAttempt("network");
});

test("overlapping dispatches and multiple requests cannot borrow one another's HTTP failure", async (t) => {
  const replies = [];
  const server = createServer((_req, res) => {
    replies.push(res);
  });
  const proxy = await startEgressProxy(await listen(server));
  t.after(async () => {
    await proxy.close();
    server.closeAllConnections();
    await new Promise((r) => server.close(r));
  });
  proxy.beginAttempt("a");
  const first = fetch(proxy.endpoint);
  while (replies.length !== 1) await new Promise((r) => setTimeout(r, 1));
  proxy.beginAttempt("b");
  const second = fetch(proxy.endpoint);
  while (replies.length !== 2) await new Promise((r) => setTimeout(r, 1));
  replies[1].writeHead(503).end();
  replies[0].writeHead(401).end();
  await Promise.all([first, second]);
  assert.equal(proxy.takeFailure("a"), undefined);
  assert.equal(proxy.takeFailure("b"), undefined);
  proxy.endAttempt("a");
  proxy.endAttempt("b");
  proxy.beginAttempt("multiple");
  const third = fetch(proxy.endpoint);
  while (replies.length !== 3) await new Promise((r) => setTimeout(r, 1));
  replies[2].writeHead(401).end();
  await third;
  const fourth = fetch(proxy.endpoint);
  while (replies.length !== 4) await new Promise((r) => setTimeout(r, 1));
  replies[3].writeHead(200).end();
  await fourth;
  assert.equal(proxy.takeFailure("multiple"), undefined);
  proxy.endAttempt("multiple");
});

test(
  "composed provider emits one request-owned error and never reuses a previous 401",
  { timeout: 45000 },
  async (t) => {
    const root = await realpath(
      await mkdtemp(join(tmpdir(), "rss-egress-attempt-")),
    );
    let status = 401;
    const server = createServer((_req, res) =>
      res.writeHead(status).end('{"error":{"message":"CANARY_PRIVATE_ERROR"}}'),
    );
    const endpoint = await listen(server);
    const namespace = fixtureSession().namespace;
    const configuration = {
      namespace,
      provider: "codex",
      config: { id: "custom", revision: "1" },
      workingDirectory: root,
      permissions: "tools_disabled",
    };
    const port = await createProvider({
      configuration,
      activation: {
        namespace,
        secret: "synthetic-fixture-key",
        local: {
          workingDirectory: root,
          nativeDirectory: join(root, "native"),
        },
        connection: {
          schemaVersion: 5,
          kind: "connection",
          connectionId: "custom",
          configRevision: 1,
          name: "fixture",
          provider: "codex",
          profile: "conversation",
          status: "ready",
          source: { type: "custom_api", apiUrl: endpoint, model: "fixture" },
        },
      },
    });
    const budget = () => ({
      timeoutMs: 15000,
      signal: AbortSignal.timeout(15000),
    });
    t.after(async () => {
      await port.close(budget());
      server.closeAllConnections();
      await new Promise((r) => server.close(r));
      await rm(root, { recursive: true, force: true });
    });
    const opened = unwrap(await port.createSession(configuration, budget()));
    for (const [id, expected] of [
      ["auth", "authentication_required"],
      ["server", "unavailable"],
      ["transport", "unavailable"],
    ]) {
      if (id === "server") status = 500;
      if (id === "transport") {
        server.closeAllConnections();
        await new Promise((r) => server.close(r));
      }
      const command = {
        ...fixtureCommand(id),
        sessionId: namespace.sessionId,
        expiresAtMs: Date.now() + 60000,
      };
      const attempt = {
        ...fixtureAttempt(opened.binding, command),
        nativeThreadId: opened.binding.nativeThreadId,
      };
      const sent = await port.dispatch(
        opened.binding,
        command,
        attempt,
        budget(),
      );
      assert.equal(sent.certainty, "submitted", JSON.stringify({ id, sent }));
      const events = [];
      for await (const event of port.observe(sent.binding, budget())) {
        events.push(event);
        if (event.body?.type === "terminal") break;
      }
      assert.deepEqual(
        events
          .filter((e) => e.body?.type === "error")
          .map((e) => e.body.failure),
        [{ code: expected, retry: "never" }],
      );
      assert.equal(events.at(-1).body.outcome, "failed");
      assert.equal(JSON.stringify(events).includes("CANARY"), false);
    }
  },
);
