import { execFileSync } from "node:child_process";
import { constants } from "node:fs";
import { open, lstat } from "node:fs/promises";
import { dirname, join } from "node:path";

/** Check and read the same file description; never reopen a checked pathname. */
export async function readPrivateFile(
  path: string,
  limit: number,
): Promise<string> {
  if (process.platform === "win32") {
    if (limit > 65536) throw new Error("private file limit");
    const bytes = execFileSync(
      join(dirname(process.execPath), "rss-private-storage.exe"),
      ["read", path],
      {
        timeout: 5000,
        maxBuffer: 65536,
        windowsHide: true,
        stdio: ["ignore", "pipe", "ignore"],
      },
    );
    if (bytes.length > limit) throw new Error("private file limit");
    return bytes.toString("utf8");
  }
  const directory = await lstat(dirname(path));
  if (
    !directory.isDirectory() ||
    (directory.mode & 0o077) !== 0 ||
    (process.getuid && directory.uid !== process.getuid())
  )
    throw new Error("private file directory ownership");
  const file = await open(
    path,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const stat = await file.stat();
    if (
      !stat.isFile() ||
      (stat.mode & 0o077) !== 0 ||
      (process.getuid && stat.uid !== process.getuid()) ||
      stat.size > limit
    )
      throw new Error("file must be private, owned and bounded");
    const buffer = Buffer.alloc(limit + 1);
    let length = 0;
    while (length <= limit) {
      const { bytesRead } = await file.read(
        buffer,
        length,
        buffer.length - length,
        null,
      );
      if (!bytesRead) return buffer.subarray(0, length).toString("utf8");
      length += bytesRead;
    }
    throw new Error("private file size limit");
  } finally {
    await file.close();
  }
}

export async function privateDirectory(path: string): Promise<void> {
  if (process.platform === "win32") {
    execFileSync(
      join(dirname(process.execPath), "rss-private-storage.exe"),
      ["directory", path],
      { timeout: 5000, windowsHide: true, stdio: "ignore" },
    );
    return;
  }
  const { mkdir } = await import("node:fs/promises");
  await mkdir(path, { recursive: true, mode: 0o700 });
  const stat = await lstat(path);
  if (
    !stat.isDirectory() ||
    stat.isSymbolicLink() ||
    (stat.mode & 0o077) !== 0 ||
    stat.uid !== process.getuid?.()
  )
    throw new Error("private directory required");
}
