import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { frontendClosure } from "../../scripts/frontend-closure.mjs";
export default defineConfig({
  plugins: [react(), frontendClosure("ui")],
  build: {
    lib: {
      entry: "runtime.ts",
      formats: ["es"],
      fileName: "ui",
      cssFileName: "style",
    },
    rollupOptions: {
      external: [
        "react",
        "react-dom",
        "react/jsx-runtime",
        "antd",
        "@ant-design/icons",
      ],
    },
  },
});
