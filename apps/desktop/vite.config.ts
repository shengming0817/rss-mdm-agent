import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { frontendClosure } from "../../scripts/frontend-closure.mjs";
export default defineConfig(async ({ command }) => {
  const fixture = process.env.RSS_DESKTOP_ASSEMBLY === "fixture";
  if (fixture && command !== "serve")
    throw new Error("fixture is development-only");
  const owner = fixture
    ? await (
        await import(
          new URL("../../tests/assistant/server.mjs", import.meta.url).href
        )
      ).createFixture()
    : undefined;
  if (owner) await owner.seed();
  return {
    plugins: [
      react(),
      frontendClosure("desktop"),
      ...(owner
        ? [
            owner.plugin,
            { name: "fixture-cleanup", closeBundle: () => owner.close() },
          ]
        : []),
    ],
    clearScreen: false,
    server: {
      host: "127.0.0.1",
      port: 1420,
      strictPort: true,
      watch: { ignored: ["**/src-tauri/**"] },
    },
  };
});
