/** Transport to the fixed native file-protection owner; no OS permission policy lives here. */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
function invoke(arguments_: string[], output = false): Buffer {
  try {
    return (
      execFileSync(
        join(
          dirname(process.execPath),
          process.platform === "win32"
            ? "rss-private-storage.exe"
            : "rss-private-storage",
        ),
        arguments_,
        {
          timeout: 5000,
          maxBuffer: 65536,
          windowsHide: true,
          stdio: ["ignore", output ? "pipe" : "ignore", "ignore"],
        },
      ) ?? Buffer.alloc(0)
    );
  } catch {
    throw new Error("private_storage_unavailable");
  }
}
export function readPrivateFile(path: string, limit: number): string {
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 65536)
    throw new Error("private_storage_unavailable");
  return invoke(["read", path, String(limit)], true).toString("utf8");
}
export function privateDirectory(path: string): void {
  invoke(["directory", path]);
}
export function validateDirectory(path: string): void {
  invoke(["validate-directory", path]);
}
export function validateFile(path: string): void {
  invoke(["validate-file", path]);
}
export function validateOptionalFile(path: string): void {
  invoke(["validate-if-present", path]);
}
export function createPrivateFile(path: string): void {
  invoke(["create-new", path]);
}
