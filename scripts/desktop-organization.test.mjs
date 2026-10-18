import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  writeFileSync,
  rmSync,
  mkdirSync,
  copyFileSync,
  existsSync,
  readFileSync,
  statSync,
  symlinkSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { IPC_VERSION } from "../packages/execution-bindings/src/service-version.mjs";
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
  const base = `RSS_AGENT_ORIGIN=https://agent.fixture.test\nRSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\n`;
  const configure = (path) =>
    writeFileSync(
      join(root, ".env"),
      base + "RSS_AGENT_CA_FILE=" + path + "\nRSS_MDM_CA_FILE=" + path + "\n",
    );
  configure(ca);
  const environment = desktopBuildEnvironment(root, {
    RSS_MDM_CA_FILE: "stale",
  });
  const desktop = JSON.parse(environment[organizationBuildInput]);
  const template = {
    version: 5,
    ipc_version: IPC_VERSION,
    signing_keys: { key: "public" },
    state_root: "/state",
    ca_file: "/old",
  };
  const installedCa = join(root, "installed-ca.pem");
  const agent = agentOrganizationConfiguration(root, template, installedCa);
  assert.equal(agent.ca, desktop.ca_pem);
  assert.equal(agent.config.ca_file, installedCa);
  assert.equal(agent.config.origin, "https://agent.fixture.test");
  assert.notEqual(agent.config.origin, desktop.origin);
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
  assert.throws(
    () => agentOrganizationConfiguration(root, template),
    /RSS_AGENT_CA_FILE/,
  );
});

test("Agent origin normalization matches desktop DNS root-dot rules", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-origin-env-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const template = {
    version: 5,
    ipc_version: IPC_VERSION,
  };

  const configure = (origin) =>
    writeFileSync(
      join(root, ".env"),
      `RSS_AGENT_ORIGIN=${origin}\nRSS_AGENT_CA_FILE=${new URL("../tests/agent-protocol/pki/root-test-ca.pem", import.meta.url).pathname}\nRSS_MDM_ORIGIN=${origin}\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\n`,
    );
  configure("https://fixture.test");
  assert.throws(
    () =>
      agentOrganizationConfiguration(
        root,
        { version: 5, ipc_version: 6 },
        join(root, "old-ca.pem"),
      ),
    /current deployment and IPC versions/,
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
    agentOrganizationConfiguration(
      root,
      template,
      join(root, "installed-ca.pem"),
    ).config.origin,
    "https://mdm.fixture.test",
  );
});

test("real Agent generator CLI preserves user-readable public inputs under restrictive umask", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-agent-cli-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  for (const name of ["agent-organization.mjs", "desktop-organization.mjs"])
    copyFileSync(new URL(name, import.meta.url), join(root, "scripts", name));
  mkdirSync(join(root, "packages/execution-bindings/src"), { recursive: true });
  copyFileSync(
    new URL(
      "../packages/execution-bindings/src/service-version.mjs",
      import.meta.url,
    ),
    join(root, "packages/execution-bindings/src/service-version.mjs"),
  );
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
    `RSS_AGENT_ORIGIN=https://agent.fixture.test\nRSS_AGENT_CA_FILE=${cert}\nRSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\nRSS_MDM_CA_FILE=${cert}\n`,
  );
  const template = join(root, "template.json"),
    output = join(root, "execution.json"),
    ca = join(root, "installed-ca.pem");
  writeFileSync(
    template,
    JSON.stringify({
      version: 5,
      ipc_version: IPC_VERSION,
      state_root: "/state",
    }),
  );
  let entry = join(root, "scripts", "agent-organization.mjs");
  if (process.platform !== "win32") {
    symlinkSync(join(root, "scripts"), join(root, "scripts-link"), "dir");
    entry = join(root, "scripts-link", "agent-organization.mjs");
  }
  const withoutCa = join(root, "without-ca.json");
  const previousUmask =
    process.platform === "win32" ? undefined : process.umask(0o077);
  try {
    execFileSync(
      process.execPath,
      [entry, "--config", template, "--output", output, "--ca-output", ca],
      { stdio: "ignore" },
    );
    writeFileSync(
      join(root, ".env"),
      `RSS_AGENT_ORIGIN=https://agent.fixture.test\nRSS_MDM_ORIGIN=https://fixture.test\nRSS_MDM_TENANT_ID=aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\nRSS_MDM_ORGANIZATION_LABEL=Fixture\n`,
    );
    assert.throws(() =>
      execFileSync(
        process.execPath,
        [entry, "--config", template, "--output", withoutCa],
        { stdio: "ignore" },
      ),
    );
  } finally {
    if (previousUmask !== undefined) process.umask(previousUmask);
  }
  const config = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(config.ca_file, ca);
  assert.equal(existsSync(withoutCa), false);
  assert.equal(config.state_root, "/state");
  assert.match(readFileSync(ca, "utf8"), /BEGIN CERTIFICATE/);
  if (process.platform !== "win32") {
    assert.equal(statSync(output).mode & 0o777, 0o644);
    assert.equal(statSync(ca).mode & 0o777, 0o644);
  }
});

test("managed package formats use explicit machine service owners and escaped fixed inputs", async () => {
  const { msiSource, msixManifest } = await import("./windows-packages.mjs");
  const msi = msiSource({
    version: "1.2.3",
    target: "windows-amd64",
    id: "fixed-id",
    payload: "C:/build&sign/payload",
    files: ["rss-execution-service.exe", "candidate.json", "runtime/node.exe"],
  });
  assert.ok(msi.includes('Scope="perMachine"'));
  assert.ok(msi.includes('Execute="deferred" Impersonate="no"'));
  assert.ok(msi.includes("C:/build&amp;sign/payload"));
  assert.ok(msi.includes("--installation apply"));
  assert.ok(msi.includes('Execute="rollback"'));
  assert.ok(msi.includes("--installation msi-rollback"));
  assert.ok(msi.includes('Schedule="afterInstallExecute"'));
  assert.ok(
    msi.includes(
      'Condition="REMOVE=&quot;ALL&quot; AND NOT UPGRADINGPRODUCTCODE"',
    ),
  );
  assert.ok(msi.includes('BinaryRef="InstallationOwner"'));

  const msix = msixManifest({
    version: "1.2.3",
    architecture: "arm64",
    publisher: "CN=RSS & Test",
  });
  assert.ok(msix.includes('Publisher="CN=RSS &amp; Test"'));
  assert.ok(
    msix.includes(
      'Name="RssExecution" StartupType="auto" StartAccount="localSystem"',
    ),
  );
  assert.ok(msix.includes('Arguments="--packaged-service"'));
  assert.ok(!msix.includes("PackageDependency"));
  assert.throws(() =>
    msixManifest({
      version: "latest",
      architecture: "x64",
      publisher: "CN=RSS",
    }),
  );
});
