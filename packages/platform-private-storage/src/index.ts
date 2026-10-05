/** Asynchronous transport to the fixed native file-protection owner. */
import { execFile } from "node:child_process";
import { dirname, join } from "node:path";
export class PrivateStorageRejected extends Error {
  constructor() {
    super("private_storage_unavailable");
  }
}
export class PrivateStorageUnavailable extends Error {
  constructor(readonly reason: "absent" | "timeout" | "transport") {
    super("private_storage_unavailable");
  }
}
function invoke(arguments_: string[]): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    execFile(
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
        encoding: "buffer",
        killSignal: "SIGKILL",
      },
      (error, stdout) => {
        if (!error) {
          resolve(stdout);
          return;
        }
        if (error.code === 1) reject(new PrivateStorageRejected());
        else
          reject(
            new PrivateStorageUnavailable(
              error.code === "ENOENT"
                ? "absent"
                : error.killed
                  ? "timeout"
                  : "transport",
            ),
          );
      },
    );
  });
}
export async function readPrivateFile(
  path: string,
  limit: number,
): Promise<string> {
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 65536)
    throw new PrivateStorageRejected();
  return (await invoke(["read", path, String(limit)])).toString("utf8");
}
export async function privateDirectory(path: string): Promise<void> {
  await invoke(["directory", path]);
}
export async function validateDirectory(path: string): Promise<void> {
  await invoke(["validate-directory", path]);
}
export async function validateFile(path: string): Promise<void> {
  await invoke(["validate-file", path]);
}
export async function validateOptionalFile(path: string): Promise<void> {
  await invoke(["validate-if-present", path]);
}
export async function createPrivateFile(path: string): Promise<void> {
  await invoke(["create-new", path]);
}
export async function validateSingleLinkFile(path: string): Promise<void> {
  await invoke(["validate-single-link", path]);
}
export async function validateSidecars(path: string): Promise<void> {
  await invoke(["validate-sidecars", path]);
}
