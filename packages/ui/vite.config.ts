import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
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
