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
import {
  freezeCandidate,
  verifyCandidate,
  verifyNativeRuntime,
} from "./native-candidate.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "agent-candidate-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const artifacts = join(root, "artifacts");
  mkdirSync(artifacts);
  const binaries = Object.fromEntries(
    ["service", "backend", "desktop", "securityProbe", "untrustedProbe"].map(
      (name) => {
        const path = join(artifacts, name);
        copyFileSync("/bin/sh", path);
        return [name, path];
      },
    ),
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
