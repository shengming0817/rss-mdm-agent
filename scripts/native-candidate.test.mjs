import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  copyFileSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { runtimeTreeSha256 } from "./ai-host-artifacts.mjs";
import { startModelFixture } from "../tests/desktop/model-fixture.mjs";
import {
  freezeCandidate,
  verifyCandidate,
  verifyNativeRuntime,
  nativeProductModules,
} from "./native-candidate.mjs";

test("native model oracle requires the original software request to await human confirmation", async (t) => {
  for (const [request, state, confirmation, expectedStatus] of [
    ["review-request", "awaitingConfirmation", null, 200],
    ["review-request", "ready", null, 500],
    ["another-request", "awaitingConfirmation", null, 500],
    ["review-request", "awaitingConfirmation", { approved: true }, 500],
  ]) {
    const fixture = await startModelFixture();
    t.after(() => fixture.close());
    fixture.accept();
    fixture.facts.request = "original-human-request";
    const result = (value) =>
      JSON.stringify({
        content: [
          {
            type: "text",
            text: JSON.stringify({ status: "ok", result: value }),
          },
        ],
      });
    const response = await fetch(fixture.url + "/responses", {
      method: "POST",
      headers: {
        authorization: `Bearer ${fixture.secret}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        model: "native-golden-model",
        input: [
          { role: "user", content: "GOLDEN_REVIEW" },
          {
            type: "function_call_output",
            call_id: "REVIEW-tools",
            output: result({
              tools: [
                {
                  name: "exact-discovered-tool",
                  inputSchema: {
                    properties: {
                      locator: { properties: { after: { type: "null" } } },
                    },
                  },
                },
              ],
            }),
          },
          {
            type: "function_call_output",
            call_id: "REVIEW-invoke",
            output: result({ operationRequestId: "review-request" }),
          },
          {
            type: "function_call_output",
            call_id: "REVIEW-status",
            output: result({
              kind: "preparation",
              value: {
                action: { kind: "policy", choice: { request } },
                state,
                risk: { level: "two" },
                confirmation,
              },
            }),
          },
        ],
      }),
    });
    await response.text();
    assert.equal(response.status, expectedStatus);
    assert.equal(fixture.facts.reviewRequest, "review-request");
    assert.deepEqual(
      fixture.facts.completions,
      expectedStatus === 200 ? ["REVIEW"] : [],
    );
  }
});

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "agent-candidate-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const artifacts = join(root, "artifacts");
  mkdirSync(artifacts);
  const binaries = Object.fromEntries(
    [
      "service",
      "backend",
      "desktop",
      "platformSso",
      "securityProbe",
      "untrustedProbe",
    ].map((name) => {
      const path = join(artifacts, name);
      copyFileSync("/bin/sh", path);
      return [name, path];
    }),
  );
  const runtime = join(artifacts, "runtime");
  mkdirSync(join(runtime, "bin"), { recursive: true });
  mkdirSync(join(runtime, "node_modules"));
  for (const name of [
    "NODE-LICENSE",
    "package.json",
    "pnpm-lock.yaml",
    "worker-manifest.json",
  ])
    writeFileSync(join(runtime, name), "{}");
  writeFileSync(join(runtime, "bin/node"), "fixed runtime");
  writeFileSync(
    join(runtime, "manifest.json"),
    JSON.stringify({
      kind: "development",
      status: "passed",
      developmentFingerprint: "frozen-build-source",
      runtimeTreeSha256: runtimeTreeSha256(runtime),
    }),
  );
  for (const name of ["README.md", "Cargo.lock", "pnpm-lock.yaml"])
    writeFileSync(join(root, name), "original");
  const { engines, packageManager } = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url)),
  );
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({ engines, packageManager }),
  );
  writeFileSync(join(root, ".gitignore"), "/artifacts/\n");
  const git = (...args) =>
    execFileSync("/usr/bin/git", args, { cwd: root, stdio: "pipe" });
  git("init", "--quiet");
  const commit = () => {
    git("add", ".");
    git(
      "-c",
      "user.name=Candidate Test",
      "-c",
      "user.email=candidate@example.invalid",
      "commit",
      "--quiet",
      "-m",
      "fixture",
    );
  };
  commit();
  const candidate = freezeCandidate(root, { ...binaries, runtime });
  const file = join(artifacts, "candidate.json");
  writeFileSync(file, JSON.stringify(candidate));
  return { root, runtime, binaries, candidate, file, commit };
}

const native = { skip: process.platform !== "darwin" };
test(
  "fixed candidate preserves build provenance across unrelated source changes",
  native,
  (t) => {
    const { root, candidate, file, commit } = fixture(t);
    assert.equal(candidate.binaries.platformSso.signature.teamId, null);
    assert.ok(candidate.binaries.platformSso.signature.cdhash);
    const verify = () => assert.deepEqual(verifyCandidate(file), candidate);
    verify();
    writeFileSync(join(root, "README.md"), "documentation changed");
    verify();
    commit();
    verify();
    writeFileSync(join(root, "unrelated.rs"), "new source");
    for (const name of ["Cargo.lock", "pnpm-lock.yaml"])
      writeFileSync(join(root, name), "changed lock");
    verify();

    // No checkout, lock files, package manager or shell is needed to verify fixed bytes.
    const cwd = mkdtempSync(join(tmpdir(), "candidate-verification-"));
    t.after(() => rmSync(cwd, { recursive: true, force: true }));
    const declared = structuredClone(candidate);
    declared.source.node = "v22.0.0";
    declared.source.pnpm = "9.0.0";
    writeFileSync(file, JSON.stringify(declared));
    const actual = JSON.parse(
      execFileSync(
        process.execPath,
        [
          fileURLToPath(new URL("native-candidate.mjs", import.meta.url)),
          "verify",
          file,
        ],
        {
          cwd,
          env: { ...process.env, PATH: "" },
          encoding: "utf8",
        },
      ),
    );
    assert.deepEqual(actual, declared);
  },
);

test(
  "fixed candidate rejects every replaced image, code identity and incompatible target",
  native,
  (t) => {
    const { candidate, file, binaries } = fixture(t);
    for (const field of ["platform", "architecture"])
      assert.throws(() => {
        writeFileSync(
          file,
          JSON.stringify({ ...candidate, [field]: "different-target" }),
        );
        verifyCandidate(file);
      });
    for (const name of Object.keys(binaries)) {
      for (const [key, value] of [
        ["cdhash", "0".repeat(40)],
        ["mode", "different-identity"],
      ]) {
        const changed = structuredClone(candidate);
        changed.binaries[name].signature[key] = value;
        writeFileSync(file, JSON.stringify(changed));
        assert.throws(() => verifyCandidate(file), /candidate image changed/);
      }
      writeFileSync(file, JSON.stringify(candidate));
      writeFileSync(binaries[name], "replaced bytes");
      assert.throws(() => verifyCandidate(file));
      copyFileSync("/bin/sh", binaries[name]);
    }
    assert.deepEqual(verifyCandidate(file), candidate);
  },
);

test(
  "native fixed runtime ignores checkout fingerprints and rejects changed or substituted runtimes",
  native,
  (t) => {
    const { root, runtime, candidate, file } = fixture(t);
    const manifest = readFileSync(join(runtime, "manifest.json"));
    assert.equal(
      verifyNativeRuntime(join(root, "absent-checkout"), runtime, file)
        .developmentFingerprint,
      "frozen-build-source",
    );
    const other = join(root, "other-runtime");
    mkdirSync(other);
    writeFileSync(join(other, "manifest.json"), manifest);
    assert.throws(
      () => verifyNativeRuntime(root, other, file),
      /candidate runtime mismatch/,
    );
    writeFileSync(join(runtime, "manifest.json"), "substituted manifest");
    assert.throws(() => verifyCandidate(file));
    writeFileSync(join(runtime, "manifest.json"), manifest);
    writeFileSync(join(runtime, "bin/node"), "substituted runtime");
    assert.throws(
      () => verifyNativeRuntime(root, runtime, file),
      /runtime tree integrity mismatch/,
    );
    writeFileSync(join(runtime, "bin/node"), "fixed runtime");
    assert.deepEqual(verifyCandidate(file), candidate);
  },
);

test(
  "fixed security probes execute frozen modules despite different checkout behavior",
  native,
  async (t) => {
    const { root, runtime, binaries, file } = fixture(t);
    const install = (packages, identity, absent) => {
      for (const [name, files] of Object.entries({
        "ai-host": {
          "index.js": `export function createHost(){return ${JSON.stringify(identity)}}`,
          "process.js": `export function scopeAbsentWithin(){return ${absent}}; export class WorkerPort {source=${JSON.stringify(identity)}}`,
        },
        "ai-store-sqlite": {
          "index.js": `export function openSqliteStore(){return ${JSON.stringify(identity)}}`,
        },
      })) {
        mkdirSync(join(packages, name, "dist"), { recursive: true });
        writeFileSync(
          join(packages, name, "package.json"),
          JSON.stringify({ type: "module" }),
        );
        for (const [leaf, source] of Object.entries(files))
          writeFileSync(join(packages, name, "dist", leaf), source);
      }
    };
    const checkout = join(root, "packages");
    install(checkout, "candidate-A", false);
    install(join(runtime, "node_modules/@rss-mdm-agent"), "candidate-A", false);
    const manifest = JSON.parse(readFileSync(join(runtime, "manifest.json")));
    manifest.runtimeTreeSha256 = runtimeTreeSha256(runtime);
    writeFileSync(join(runtime, "manifest.json"), JSON.stringify(manifest));
    const frozen = freezeCandidate(root, { ...binaries, runtime });
    writeFileSync(file, JSON.stringify(frozen));
    install(checkout, "checkout-B", true);
    assert.deepEqual(verifyCandidate(file), frozen);
    const fixedModules = nativeProductModules(root, frozen.runtime.path);
    const currentModules = nativeProductModules(root);
    for (const [modules, identity, absent] of [
      [fixedModules, "candidate-A", false],
      [currentModules, "checkout-B", true],
    ]) {
      const { createHost } = await import(modules.host);
      const { openSqliteStore } = await import(modules.store);
      const { WorkerPort, scopeAbsentWithin } = await import(modules.process);
      assert.equal(createHost(), identity);
      assert.equal(openSqliteStore(), identity);
      assert.equal(new WorkerPort().source, identity);
      assert.equal(scopeAbsentWithin(), absent);
    }
    const missing = nativeProductModules(root, join(root, "missing-runtime"));
    for (const url of Object.values(missing))
      await assert.rejects(import(url), { code: "ERR_MODULE_NOT_FOUND" });
  },
);
