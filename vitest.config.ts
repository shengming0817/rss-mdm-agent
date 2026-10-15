import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "happy-dom",
    setupFiles: ["packages/ui/tests/setup.ts"],
    include: [
      "packages/ui/tests/**/*.test.ts",
      "apps/desktop/src/**/*.test.ts",
    ],
    restoreMocks: true,
  },
});
