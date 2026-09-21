import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
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

test("egress reports closed HTTP categories without retaining provider error bodies", async (t) => {
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
    await (await fetch(proxy.endpoint)).text();
    assert.equal(proxy.failure, expected);
    assert.equal(JSON.stringify(proxy).includes("CANARY"), false);
  }
});
