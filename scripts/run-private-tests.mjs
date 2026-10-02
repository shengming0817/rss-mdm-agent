import { runPrivateRuntime } from "./ai-host-artifacts.mjs";
import { join } from "node:path";
import { readdirSync } from "node:fs";
const tests = process.argv.slice(2).flatMap((directory) =>
  readdirSync(directory)
    .filter((name) => name.endsWith(".test.mjs"))
    .map((name) => join(directory, name)),
);
if (!tests.length) throw new Error("private runtime tests required");
process.exitCode = runPrivateRuntime(process.cwd(), ["--test", ...tests]);
