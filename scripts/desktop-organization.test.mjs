import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  writeFileSync,
  rmSync,
  mkdirSync,
  copyFileSync,
  readFileSync,
  statSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { agentOrganizationConfiguration } from "./agent-organization.mjs";
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

test("desktop and Agent consume the same env CA without embedding paths or private keys", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-ca-env-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const ca = join(root, "ca.pem"),
    key = join(root, "ca.key");
  execFileSync(
    "openssl",
    [
      "req",
      "-x509",
      "-newkey",
      "rsa:2048",
      "-nodes",
      "-days",
      "1",
      "-subj",
      "/CN=RSS Test CA",
      "-addext",
      "basicConstraints=critical,CA:TRUE",
      "-keyout",
      key,
      "-out",
      ca,
    ],
    { stdio: "ignore" },
  );
  const base =
    "RSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\n";
  const configure = (path) =>
    writeFileSync(join(root, ".env"), base + "RSS_MDM_CA_FILE=" + path + "\n");
  configure(ca);
  const environment = desktopBuildEnvironment(root, {
    RSS_MDM_CA_FILE: "stale",
  });
  const desktop = JSON.parse(environment[organizationBuildInput]);
  const template = {
    version: 2,
    ipc_version: 6,
    signing_keys: { key: "public" },
    state_root: "/state",
    ca_file: "/old",
  };
  const installedCa = join(root, "installed-ca.pem");
  const agent = agentOrganizationConfiguration(root, template, installedCa);
  assert.equal(agent.ca, desktop.ca_pem);
  assert.equal(agent.config.ca_file, installedCa);
  assert.equal(agent.config.origin, desktop.origin);
  assert.equal(agent.config.tenant, desktop.tenant);
  assert.deepEqual(agent.config.signing_keys, template.signing_keys);
  assert.equal(template.ca_file, "/old");
  assert.equal(environment.RSS_MDM_CA_FILE, undefined);
  assert.ok(!environment[organizationBuildInput].includes(ca));
  assert.throws(
    () => agentOrganizationConfiguration(root, template),
    /ca-output/,
  );
  configure(key);
  assert.throws(() => desktopBuildEnvironment(root), /public certificate/);
  configure("relative.pem");
  assert.throws(() => desktopBuildEnvironment(root), /absolute/);
  configure(join(root, "missing.pem"));
  assert.throws(() => desktopBuildEnvironment(root), /public certificate/);
  configure("");
  const defaults = agentOrganizationConfiguration(root, template);
  assert.equal(defaults.ca, undefined);
  assert.equal(defaults.config.ca_file, null);
});

test("Agent origin normalization matches desktop DNS root-dot rules", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-origin-env-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const template = { version: 2, ipc_version: 6 };
  const configure = (origin) =>
    writeFileSync(
      join(root, ".env"),
      `RSS_MDM_ORIGIN=${origin}\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\n`,
    );
  for (const origin of [
    "https://MDM.EXAMPLE.:443/",
    "https://example./",
    "https://nested.mdm.example../",
  ]) {
    configure(origin);
    assert.throws(
      () => agentOrganizationConfiguration(root, template),
      /real HTTPS/,
    );
  }
  configure("https://MDM.fixture.test.:443/");
  assert.equal(
    agentOrganizationConfiguration(root, template).config.origin,
    "https://mdm.fixture.test",
  );
});

test("real Agent generator CLI preserves user-readable public inputs under restrictive umask", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-agent-cli-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  for (const name of ["agent-organization.mjs", "desktop-organization.mjs"])
    copyFileSync(new URL(name, import.meta.url), join(root, "scripts", name));
  const cert = join(root, "source-ca.pem"),
    key = join(root, "source-ca.key");
  execFileSync(
    "openssl",
    [
      "req",
      "-x509",
      "-newkey",
      "rsa:2048",
      "-nodes",
      "-days",
      "1",
      "-subj",
      "/CN=RSS Test CA",
      "-addext",
      "basicConstraints=critical,CA:TRUE",
      "-keyout",
      key,
      "-out",
      cert,
    ],
    { stdio: "ignore" },
  );
  writeFileSync(
    join(root, ".env"),
    `RSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\nRSS_MDM_CA_FILE=${cert}\n`,
  );
  const template = join(root, "template.json"),
    output = join(root, "execution.json"),
    ca = join(root, "installed-ca.pem");
  writeFileSync(
    template,
    JSON.stringify({ version: 2, ipc_version: 6, state_root: "/state" }),
  );
  const previousUmask =
    process.platform === "win32" ? undefined : process.umask(0o077);
  try {
    execFileSync(
      process.execPath,
      [
        join(root, "scripts", "agent-organization.mjs"),
        "--config",
        template,
        "--output",
        output,
        "--ca-output",
        ca,
      ],
      { stdio: "ignore" },
    );
  } finally {
    if (previousUmask !== undefined) process.umask(previousUmask);
  }
  const config = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(config.ca_file, ca);
  assert.equal(config.state_root, "/state");
  assert.match(readFileSync(ca, "utf8"), /BEGIN CERTIFICATE/);
  if (process.platform !== "win32") {
    assert.equal(statSync(output).mode & 0o777, 0o644);
    assert.equal(statSync(ca).mode & 0o777, 0o644);
  }
});
