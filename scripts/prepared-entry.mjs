import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { entry } from "./preparation-entries.mjs";
import { withPreparation, root, preparedArtifact } from "./preparation.mjs";
import { runCommand } from "./ci-plan.mjs";
export async function runEntry(name, args = []) {
  const { requirements, commands } = entry(name, args);
  return withPreparation(requirements, async (env, signal, nested) => {
    for (const key of requirements)
      if (key.startsWith("node:")) preparedArtifact(key);
    for (const [command, original] of commands) {
      // Test/type gates have multiple commands; only the last accepts caller arguments.
      const argv =
        commands.at(-1)[1] === original ? [...original, ...args] : original;
      const result = await runCommand(command, argv, {
        cwd: root,
        env,
        signal,
        stdio: "inherit",
        ...(nested ? { detached: false } : {}),
      });
      if (result.status !== 0) return result.status ?? 1;
    }
    return 0;
  });
}
if (
  process.argv[1] &&
  fileURLToPath(import.meta.url) === resolve(process.argv[1])
)
  process.exitCode = await runEntry(process.argv[2], process.argv.slice(3));
