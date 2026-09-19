#!/usr/bin/env node
import { startLocalApp } from "./index.js";
import { ConfigurationError } from "./configuration.js";
if (process.argv.length !== 3 || process.argv[2] === "--help") {
  process.stdout.write(
    "Usage: rss-ai-host /absolute/path/to/private-configuration.json\n",
  );
  process.exitCode = process.argv[2] === "--help" ? 0 : 2;
} else {
  try {
    const app = await startLocalApp(process.argv[2]);
    let stopping = false;
    const stop = async () => {
      if (stopping) return;
      stopping = true;
      try {
        await app.close();
      } catch {
        process.stderr.write("AI Host cleanup incomplete\n");
        process.exitCode = 1;
        stopping = false;
      }
    };
    process.on("SIGTERM", () => {
      void stop();
    });
    process.on("SIGINT", () => {
      void stop();
    });
  } catch (error) {
    process.stderr.write(
      `AI Host could not start: ${error instanceof ConfigurationError ? error.code : "startup_failed"}\n`,
    );
    process.exitCode = 1;
  }
}
