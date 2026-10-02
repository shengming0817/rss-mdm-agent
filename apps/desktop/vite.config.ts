import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
export default defineConfig(async ({ command }) => {
  const fixture = process.env.RSS_DESKTOP_ASSEMBLY === "fixture";
  if (fixture && command !== "serve")
    throw new Error("fixture is development-only");
  const owner = fixture
    ? await (await import("../../tests/assistant/server.mjs")).createFixture()
    : undefined;
  if (owner) await owner.seed();
  return {
    plugins: [
      vue(),
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
