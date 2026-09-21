#!/usr/bin/env node
import { startLocalApp } from "./index.js";
import { ConfigurationError } from "./configuration.js";
import type { HostProcessDiagnostic } from "@rss-mdm-agent/ai-contract";
const diagnostic = (code: HostProcessDiagnostic["code"]) => {
  const frame: HostProcessDiagnostic = {
    schemaVersion: 5,
    kind: "hostProcessDiagnostic",
    code,
  };
  process.stderr.write(JSON.stringify(frame) + "\n");
};
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
        diagnostic("cleanup_incomplete");
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
    const code =
      error instanceof ConfigurationError ? error.code : "startup_failed";
    diagnostic(
      code === "configuration_file" || code === "unsupported_capability"
        ? "configuration_invalid"
        : code === "startup_failed"
          ? "host_start_failed"
          : code,
    );
    process.exitCode = 1;
  }
}
