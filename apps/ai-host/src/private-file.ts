import { constants } from "node:fs";
import { open, lstat } from "node:fs/promises";
import { dirname } from "node:path";

/** Check and read the same file description; never reopen a checked pathname. */
export async function readPrivateFile(
  path: string,
  limit: number,
): Promise<string> {
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
