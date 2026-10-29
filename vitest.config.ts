import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "happy-dom",
    setupFiles: ["packages/ui/tests/setup.ts"],
    include: [
      "packages/ui/tests/**/*.test.{ts,tsx}",
      "apps/desktop/src/**/*.test.{ts,tsx}",
    ],
    restoreMocks: true,
  },
});
