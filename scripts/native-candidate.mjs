// Fixed inputs for the existing native acceptance journeys; this is not a build or signing entry.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync, realpathSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { sourceEvidence, sha256 } from "./native-evidence.mjs";
import {
  runtimeTreeSha256,
  verifyRuntimeIntegrity,
} from "./ai-host-artifacts.mjs";
function image(path) {
  path = realpathSync(path);
  let signature;
  if (process.platform === "darwin") {
    execFileSync("/usr/bin/codesign", ["--verify", "--strict", path], {
      stdio: "pipe",
    });
    const display = spawnSync(
      "/usr/bin/codesign",
      ["-d", "--verbose=4", path],
      { encoding: "utf8" },
    );
    assert.equal(display.status, 0, "candidate code identity unavailable");
    const result = display.stderr;
    signature = {
      cdhash: /CDHash=([0-9a-f]{40})/.exec(result)?.[1],
      mode: result.includes("Signature=adhoc") ? "controlled-ad-hoc" : "signed",
    };
    assert.ok(signature.cdhash, "candidate code identity required");
  } else signature = { mode: "target-platform-pending" };
  return { path, sha256: sha256(readFileSync(path)), signature };
}
export function freezeCandidate(root, { service, backend, desktop, runtime }) {
  const binaries = Object.fromEntries(
    Object.entries({ service, backend, desktop })
      .filter(([, path]) => path)
      .map(([name, path]) => [name, image(path)]),
  );
  assert.ok(
    binaries.service && binaries.backend,
    "service and backend candidates required",
  );
  const runtimeRoot = runtime ? realpathSync(runtime) : undefined;
  return {
    version: 1,
    platform: process.platform,
    architecture: process.arch,
    source: sourceEvidence(root),
    binaries,
    ...(runtimeRoot
      ? {
          runtime: {
            path: runtimeRoot,
            sha256: runtimeTreeSha256(runtimeRoot),
            manifestSha256: sha256(
              readFileSync(resolve(runtimeRoot, "runtime-manifest.json")),
            ),
          },
        }
      : {}),
  };
}
export function verifyCandidate(root, path) {
  const candidate = JSON.parse(readFileSync(path, "utf8"));
  assert.equal(candidate.version, 1);
  assert.equal(candidate.platform, process.platform);
  assert.equal(candidate.architecture, process.arch);
  const source = sourceEvidence(root);
  for (const key of [
    "head",
    "sourceSha256",
    "cargoLockSha256",
    "pnpmLockSha256",
    "node",
    "pnpm",
  ])
    assert.equal(
      candidate.source[key],
      source[key],
      "candidate source/toolchain changed",
    );
  assert.ok(candidate.binaries.service && candidate.binaries.backend);
  for (const expected of Object.values(candidate.binaries))
    assert.deepEqual(image(expected.path), expected, "candidate image changed");
  if (candidate.runtime) {
    verifyRuntimeIntegrity(candidate.runtime.path, candidate.runtime.sha256);
    assert.equal(
      sha256(
        readFileSync(resolve(candidate.runtime.path, "runtime-manifest.json")),
      ),
      candidate.runtime.manifestSha256,
    );
  }
  return candidate;
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === new URL(import.meta.url).pathname
) {
  const [verb, ...arguments_] = process.argv.slice(2);
  if (verb === "verify" && arguments_.length === 1) {
    console.log(JSON.stringify(verifyCandidate(process.cwd(), arguments_[0])));
  } else if (verb === "freeze") {
    const options = {};
    for (let i = 0; i < arguments_.length; i += 2) {
      const name = arguments_[i]?.replace(/^--/, "");
      assert.ok(
        ["service", "backend", "desktop", "runtime", "output"].includes(name) &&
          arguments_[i + 1],
      );
      assert.ok(!Object.hasOwn(options, name));
      options[name] = resolve(arguments_[i + 1]);
    }
    assert.ok(options.output);
    writeFileSync(
      options.output,
      JSON.stringify(freezeCandidate(process.cwd(), options), null, 2) + "\n",
      { flag: "wx", mode: 0o600 },
    );
  } else throw Error("use native-candidate freeze or verify");
}
