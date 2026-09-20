import assert from "node:assert/strict";
import test from "node:test";
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { removeUnfencedSnapshots } from "../../apps/ai-host/dist/index.js";

const hex = (character) => character.repeat(64) + ".json";

test("snapshot startup cleanup removes only unfenced owned files", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-snapshot-cleanup-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = join(root, "snapshots"),
    orphan = join(directory, hex("a")),
    retained = join(directory, hex("b")),
    note = join(directory, "operator-note.json"),
    link = join(directory, hex("c"));
  await mkdir(directory, { mode: 0o700 });
  await writeFile(orphan, "orphan", { mode: 0o600 });
  await writeFile(retained, "retained", { mode: 0o600 });
  await writeFile(note, "not owned", { mode: 0o600 });
  await symlink(orphan, link);
  const artifact = new URL("file:///private/provider.js");
  artifact.searchParams.set("snapshot", retained);

  await removeUnfencedSnapshots(root, [
    { artifact: artifact.href, phase: "registered", pgid: 999999 },
  ]);

  await assert.rejects(lstat(orphan), { code: "ENOENT" });
  assert.equal(await readFile(retained, "utf8"), "retained");
  assert.equal(await readFile(note, "utf8"), "not owned");
  assert.equal((await lstat(link)).isSymbolicLink(), true);

  await removeUnfencedSnapshots(root, []);
  await assert.rejects(lstat(retained), { code: "ENOENT" });
  assert.equal(await readFile(note, "utf8"), "not owned");
  assert.equal((await lstat(link)).isSymbolicLink(), true);
});

test("snapshot startup cleanup rejects unknown fence artifacts before deleting files", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-snapshot-fence-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = join(root, "snapshots"),
    first = join(directory, hex("e")),
    second = join(directory, hex("f"));
  await mkdir(directory, { mode: 0o700 });
  await writeFile(first, "first", { mode: 0o600 });
  await writeFile(second, "second", { mode: 0o600 });
  const outside = new URL("file:///private/provider.js");
  outside.searchParams.set("snapshot", join(root, hex("0")));

  for (const artifact of ["not a URL", outside.href]) {
    await assert.rejects(removeUnfencedSnapshots(root, [{ artifact }]));
    assert.equal(await readFile(first, "utf8"), "first");
    assert.equal(await readFile(second, "utf8"), "second");
  }
});

test("snapshot startup cleanup rejects an unowned directory without deleting it", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-snapshot-unsafe-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const target = join(root, "target"),
    sentinel = join(target, hex("d"));
  await mkdir(target, { mode: 0o700 });
  await writeFile(sentinel, "keep", { mode: 0o600 });
  await symlink(target, join(root, "snapshots"));

  await assert.rejects(removeUnfencedSnapshots(root, []), {
    message: "snapshot directory ownership",
  });
  assert.equal(await readFile(sentinel, "utf8"), "keep");
});
