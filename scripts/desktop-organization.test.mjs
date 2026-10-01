import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  desktopBuildEnvironment,
  organizationBuildInput,
} from "./desktop-organization.mjs";

test("root env is required, parsed without expansion and only allowed fields are embedded", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-organization-env-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const inherited = {
    PATH: "/fixture",
    RSS_MDM_ORIGIN: "https://stale.fixture.test",
    [organizationBuildInput]: "stale",
  };
  assert.throws(
    () => desktopBuildEnvironment(root, inherited),
    /copy env.example/,
  );
  writeFileSync(
    join(root, ".env"),
    "RSS_MDM_ORIGIN=https://fixture.test\nSECRET=DO_NOT_EMBED",
  );
  assert.throws(
    () => desktopBuildEnvironment(root, inherited),
    /RSS_MDM_TENANT_ID/,
  );
  writeFileSync(
    join(root, ".env"),
    "RSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL='Literal $NAME'\nSECRET=DO_NOT_EMBED\nNODE_OPTIONS=--unexpected\nRSS_BUILD_MDM_ORGANIZATION=stale",
  );
  const env = desktopBuildEnvironment(root, inherited);
  assert.deepEqual(JSON.parse(env[organizationBuildInput]), {
    origin: "https://fixture.test",
    tenant: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    label: "Literal $NAME",
  });
  assert.equal(env.PATH, "/fixture");
  assert.equal(env.RSS_MDM_ORIGIN, undefined);
  assert.equal(env.SECRET, undefined);
  assert.equal(env.NODE_OPTIONS, undefined);
  assert.equal(inherited[organizationBuildInput], "stale");
});
