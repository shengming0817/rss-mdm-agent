import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
export const sha256 = (value) =>
  createHash("sha256").update(value).digest("hex");
export function waitForAppearance(readSnapshot, preferences, wait) {
  const materialEnabled =
    !preferences.reduceTransparency &&
    !preferences.reducedMotion &&
    !preferences.highContrast;
  return wait(async () => {
    const snapshot = await readSnapshot();
    return snapshot.materialEnabled === materialEnabled &&
      snapshot.reducedMotion === preferences.reducedMotion &&
      snapshot.highContrast === preferences.highContrast
      ? snapshot
      : false;
  });
}
export function sourceEvidence(root) {
  const git = (...args) =>
    execFileSync(process.platform === "win32" ? "git" : "/usr/bin/git", args, {
      cwd: root,
      encoding: "utf8",
    }).trim();
  const files = [
    ...new Set(
      git("ls-files", "-z", "--cached", "--others", "--exclude-standard")
        .split("\0")
        .filter(Boolean),
    ),
  ].sort();
  const hash = createHash("sha256");
  for (const file of files) {
    hash.update(file + "\0");
    try {
      hash.update(readFileSync(join(root, file)));
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      hash.update("<deleted>");
    }
  }
  return {
    head: git("rev-parse", "HEAD"),
    sourceSha256: hash.digest("hex"),
    cargoLockSha256: sha256(readFileSync(join(root, "Cargo.lock"))),
    pnpmLockSha256: sha256(readFileSync(join(root, "pnpm-lock.yaml"))),
    ...toolchainEvidence(root),
  };
}

export function toolchainEvidence(root) {
  return {
    node: process.version,
    pnpm: execFileSync(
      process.platform === "win32" ? process.execPath : "pnpm",
      process.platform === "win32"
        ? [
            process.env.npm_execpath ??
              (() => {
                throw new Error(
                  "run Windows candidate tools through pnpm exec",
                );
              })(),
            "--version",
          ]
        : ["--version"],
      {
        cwd: root,
        encoding: "utf8",
      },
    ).trim(),
  };
}

// Standalone DOM predicate serialized by WebDriver into the actual WKWebView.
// A repeated completion phrase in an older turn is never evidence for this prompt.
export function nativeReplyComplete(
  promptKey,
  expected,
  root = globalThis.document,
) {
  const timeline = root.querySelector(".rss-assistant-timeline");
  if (timeline?.getAttribute("data-busy") !== "false") return false;
  const rows = [
    ...timeline.querySelectorAll(":scope > article[data-message-key]"),
  ];
  const at = rows.findIndex(
    (row) =>
      row.getAttribute("data-message-kind") === "user" &&
      row.getAttribute("data-message-key") === promptKey,
  );
  if (at < 0) return false;
  const after = rows.slice(at + 1);
  const next = after.findIndex(
    (row) => row.getAttribute("data-message-kind") === "user",
  );
  return after
    .slice(0, next < 0 ? undefined : next)
    .some(
      (row) =>
        row.getAttribute("data-message-kind") === "assistant" &&
        row.getAttribute("data-stable") === "true" &&
        row.textContent.includes(expected),
    );
}

export function sampledNativeTask(page) {
  return page.items.find(
    (item) =>
      item.reference.kind === "sampling" &&
      item.facts.kind === "sampling" &&
      item.facts.value.latest.collectedAt != null,
  );
}
